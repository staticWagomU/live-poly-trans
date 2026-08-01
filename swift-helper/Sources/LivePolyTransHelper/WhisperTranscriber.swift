@preconcurrency import AVFAudio
import Foundation

let whisperSampleRate: Double = 16_000

/// Upper bound on queued utterance chunks awaiting inference. Each chunk is
/// up to 28 s x 16 kHz Float (~1.8 MB), so this caps queue memory at ~22 MB
/// while leaving room for inference to catch up after a slow stretch.
let maxPendingWhisperChunks = 12

/// Mic ambient noise sits well above the digital-silence floor loopback
/// audio has, so the streams cut utterances at different levels. Tune from
/// the whisper-audio RMS debug logs of a real session, not by guessing.
func whisperSilenceThresholdRMS(for stream: AudioStream) -> Double {
  switch stream {
  case .mic:
    0.01
  case .speaker:
    0.001
  }
}

@available(macOS 26.0, *)
public func runWhisperTranscription(
  stream: AudioStream,
  sourceLanguage: String?,
  targetLanguage: String?,
  segmentDirectory: String?,
  recordFile: String? = nil,
  transcriptFile: String? = nil,
  modelPath: String,
  cliPath: String
) async throws {
  helperDebugLog(
    "whisper-stream-start stream=\(stream.rawValue) source=\(sourceLanguage ?? "-") target=\(targetLanguage ?? "-") model=\(modelPath) cli=\(cliPath) record=\(recordFile ?? "-")"
  )

  guard
    let whisperFormat = AVAudioFormat(
      commonFormat: .pcmFormatFloat32,
      sampleRate: whisperSampleRate,
      channels: 1,
      interleaved: false
    )
  else {
    throw AudioFileToolsError.missingTargetFormat
  }

  let emitter = HelperEventEmitter(
    segmentWriter: SegmentWriter(directoryPath: segmentDirectory),
    transcriptWriter: JsonlFileWriter(path: transcriptFile)
  )
  let runner = try WhisperCliRunner(cliPath: cliPath, modelPath: modelPath)
  let languages = transcriptionLanguages(
    requestedLanguages: [],
    sourceLanguage: sourceLanguage,
    targetLanguage: targetLanguage
  )
  let translators = Dictionary(uniqueKeysWithValues: languages.map { language in
    (
      language,
      LiveTranslator(
        sourceLanguage: language,
        targetLanguage: oppositeLanguage(
          for: language,
          sourceLanguage: sourceLanguage,
          targetLanguage: targetLanguage
        )
      )
    )
  })

  let chunker = UtteranceChunker(
    sampleRate: whisperSampleRate,
    silenceThresholdRMS: whisperSilenceThresholdRMS(for: stream)
  )
  let pendingChunks = PendingChunkCounter()
  var chunkContinuation: AsyncStream<UtteranceChunk>.Continuation!
  // Bounded so a wedged or slower-than-realtime inference cannot grow the
  // queue (and helper memory) for the rest of the session; the oldest queued
  // chunks keep transcribing in order and the newest is dropped instead.
  let chunkStream = AsyncStream<UtteranceChunk>(
    bufferingPolicy: .bufferingOldest(maxPendingWhisperChunks)
  ) {
    chunkContinuation = $0
  }
  let droppedChunkNotice = ShutdownOnce()
  let enqueueChunk: (UtteranceChunk) -> Void = { [chunkContinuation] chunk in
    let pending = pendingChunks.increment()
    helperDebugLog(
      "whisper-chunk stream=\(stream.rawValue) segment=\(chunk.startMs)-\(chunk.durationMs) pending=\(pending)"
    )
    if pending > 4 {
      helperDebugLog("whisper-chunk-backlog stream=\(stream.rawValue) pending=\(pending)")
    }
    if case .dropped? = chunkContinuation?.yield(chunk) {
      pendingChunks.decrement()
      helperDebugLog(
        "whisper-chunk-dropped-backlog stream=\(stream.rawValue) segment=\(chunk.startMs)-\(chunk.durationMs)"
      )
      // Plain (non-debug) stderr surfaces in the UI as a helper-error event.
      droppedChunkNotice.run {
        FileHandle.standardError.write(Data(
          "Transcription is falling behind; some audio was skipped. A smaller Whisper model keeps up better.\n"
            .utf8
        ))
      }
    }
  }

  let inputSource = try await makeWhisperInputSource(
    stream: stream,
    recordFile: recordFile,
    whisperFormat: whisperFormat
  )
  installWhisperShutdownHandlers(
    stream: stream,
    inputSource: inputSource,
    runner: runner,
    emitter: emitter
  )
  await emitter.emitStatus(StatusEvent(stream: stream, state: "control-ready"))

  // Chunks transcribe strictly in arrival order so final bubbles never
  // reorder; whisper inference for the current chunk runs while the next
  // utterance is still being captured.
  let consumer = Task {
    for await chunk in chunkStream {
      defer { pendingChunks.decrement() }
      let segment = "\(chunk.startMs)-\(chunk.durationMs)"
      do {
        let startedAt = Date()
        let result = try await runner.transcribe(chunk, sampleRate: whisperSampleRate)
        let elapsedMs = Int(Date().timeIntervalSince(startedAt) * 1000)
        helperDebugLog(
          "whisper-transcribed stream=\(stream.rawValue) segment=\(segment) elapsedMs=\(elapsedMs) language=\(result.result.language)"
        )

        guard
          let candidate = whisperTranscriptCandidate(
            result: result,
            chunkStartMs: chunk.startMs,
            chunkDurationMs: chunk.durationMs,
            sourceLanguage: sourceLanguage,
            targetLanguage: targetLanguage
          )
        else {
          helperDebugLog("whisper-chunk-dropped stream=\(stream.rawValue) segment=\(segment)")
          continue
        }

        await emitFinalTranscript(
          candidate,
          stream: stream,
          sourceLanguage: sourceLanguage,
          targetLanguage: targetLanguage,
          translators: translators,
          emitter: emitter
        )
      } catch {
        helperDebugLog(
          "whisper-chunk-error stream=\(stream.rawValue) segment=\(segment) error=\(diagnosticDescription(for: error))"
        )
      }
    }
  }

  do {
    var bufferCount = 0
    for try await captured in inputSource.sequence {
      let samples = floatSamples(from: captured.buffer)

      bufferCount += 1
      if bufferCount % 64 == 1 {
        helperDebugLog(
          "whisper-audio stream=\(stream.rawValue) rms=\(String(format: "%.6f", audioSignalRMS(samples))) threshold=\(whisperSilenceThresholdRMS(for: stream))"
        )
      }

      for chunk in chunker.consume(samples) {
        enqueueChunk(chunk)
      }
    }

    // Input ended (stdin EOF or SIGTERM stopped the capture); the pending
    // utterance still gets one last inference before the stream closes.
    if let chunk = chunker.flush() {
      enqueueChunk(chunk)
    }
    chunkContinuation.finish()
    await consumer.value
    inputSource.recordingSink.stopRecording()
    runner.cleanup()
    helperDebugLog("whisper-stream-finished stream=\(stream.rawValue)")
  } catch {
    helperDebugLog("whisper-stream-error stream=\(stream.rawValue) error=\(diagnosticDescription(for: error))")
    chunkContinuation.finish()
    consumer.cancel()
    // Cancellation kills the in-flight whisper-cli via runUntilExit's
    // cancellation handler; wait for the consumer so cleanup() does not
    // delete the temp directory under a still-running child.
    await consumer.value
    inputSource.cleanup()
    inputSource.recordingSink.stopRecording()
    runner.cleanup()
    throw error
  }
}

@available(macOS 26.0, *)
func makeWhisperInputSource(
  stream: AudioStream,
  recordFile: String?,
  whisperFormat: AVAudioFormat
) async throws -> AudioInputSource<CapturedAudioBuffer> {
  switch stream {
  case .mic:
    return try await makeMicrophoneCaptureSource(
      recordFile: recordFile,
      targetFormat: { _ in whisperFormat },
      transform: { $0 }
    )
  case .speaker:
    let speakerInput = try await SpeakerTapInput()
    let recordingSink = RecordingSink(
      sourceFormat: speakerInput.audioFormat,
      initialRecorder: try AudioRecorder(path: recordFile, sourceFormat: speakerInput.audioFormat)
    )
    let sequence = try await speakerInput.makeCaptureSequence(
      targetFormat: whisperFormat,
      recordingSink: recordingSink,
      transform: { $0 }
    )

    return AudioInputSource(sequence: sequence, recordingSink: recordingSink) {
      helperDebugLog("speaker-cleanup")
      speakerInput.stop()
    }
  }
}

func floatSamples(from buffer: AVAudioPCMBuffer) -> [Float] {
  guard let channelData = buffer.floatChannelData else {
    return []
  }

  return Array(UnsafeBufferPointer(start: channelData[0], count: Int(buffer.frameLength)))
}

final class PendingChunkCounter: @unchecked Sendable {
  private let lock = NSLock()
  private var count = 0

  func increment() -> Int {
    lock.lock()
    defer { lock.unlock() }
    count += 1
    return count
  }

  func decrement() {
    lock.lock()
    count -= 1
    lock.unlock()
  }
}

// MARK: - Graceful shutdown

let whisperShutdownOnce = ShutdownOnce()
nonisolated(unsafe) var whisperShutdownSignalSource: DispatchSourceSignal?

/// Whisper needs a longer grace than the builtin engine: the pending chunk
/// still runs one full inference (~2s plus model load) after input ends.
/// The Rust side extends its SIGKILL patience to match.
let whisperShutdownGraceSeconds: Double = 10

@available(macOS 26.0, *)
private func installWhisperShutdownHandlers(
  stream: AudioStream,
  inputSource: AudioInputSource<CapturedAudioBuffer>,
  runner: WhisperCliRunner,
  emitter: HelperEventEmitter
) {
  let trigger: @Sendable () -> Void = {
    whisperShutdownOnce.run {
      helperDebugLog("whisper-shutdown-begin stream=\(stream.rawValue)")
      // Ending the capture finishes the input sequence; the main flow then
      // flushes the pending utterance and drains the transcribe queue.
      inputSource.cleanup()
      DispatchQueue.global().asyncAfter(deadline: .now() + whisperShutdownGraceSeconds) {
        helperDebugLog("whisper-shutdown-timeout stream=\(stream.rawValue)")
        // The drain overran the grace: kill the in-flight whisper-cli so it
        // does not outlive this process, save the audio, then exit.
        runner.terminateInFlight()
        inputSource.recordingSink.stopRecording()
        runner.cleanup()
        exit(0)
      }
    }
  }

  runHelperControlLoop(
    stream: stream,
    sink: inputSource.recordingSink,
    emitter: emitter,
    onShutdown: trigger
  )

  signal(SIGTERM, SIG_IGN)
  let source = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .global())
  source.setEventHandler(handler: trigger)
  source.resume()
  whisperShutdownSignalSource = source
}
