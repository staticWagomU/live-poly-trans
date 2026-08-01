import AVFAudio
import CoreMedia
import Foundation
import Speech

@main
struct CommandLineOptionsTests {
  static func main() async throws {
    try parsesDetectLanguagesCommand()
    try parsesInstallLanguageCommand()
    try parsesUninstallLanguageCommand()
    try parsesAiServerCommand()
    try parsesWaveformCommand()
    try parsesMixCommand()
    try parsesMicStreamCommandWithLocales()
    try parsesSpeakerStreamCommandWithSegmentDirectory()
    try parsesStreamCommandWithRecordingFiles()
    try parsesStreamCommandWithWhisperEngine()
    try parsesStreamCommandWithWhisperEngineSidecar()
    try defaultsToBuiltinTranscriptionEngine()
    try rejectsUnknownTranscriptionEngine()
    try rejectsWhisperEngineWithoutModelAndCliPaths()
    try parsesTrimCommand()
    try trimsAudioFileToRange()
    try computesWaveformForAacFile()
    try mixesAacFiles()
    try treatsEmptyLanguageArgumentsAsUnset()
    try parsesStartRecordingControlLine()
    try parsesStopRecordingControlLine()
    try ignoresMalformedControlLines()
    try choosesNonClobberingRecordingFilePath()
    try readsValueAfterFlag()
    try readsRepeatedValuesAfterFlag()
    try formatsIso8601Timestamp()
    try formatsLanguageInfoWithBcp47Identifier()
    try encodesJsonLine()
    try formatsTranscriptEvent()
    try formatsTranslationEvent()
    try extractsTranscriptSpansFromSpeechAttributes()
    try detectsTranscriptLanguage()
    try buildsMeetingAiPrompts()
    try buildsRollingSummaryPrompt()
    try buildsChatPromptWithHistory()
    try decodesAiServerRequest()
    try await answersEmptyTranscriptWithoutModel()
    try requestsTranscriptConfidenceAttributes()
    try labelsSpeakerStreamAsSystemAudioSpeaker()
    try describesScreenCapturePermissionRecovery()
    try parsesCheckPermissionsCommand()
    try parsesRequestPermissionCommand()
    try rejectsUnknownPermissionKind()
    try mapsRecordPermissionOntoPermissionState()
    try mapsScreenCapturePreflightOntoPermissionState()
    try encodesPermissionStatusPayload()
    try calculatesAudioFrameLength()
    try gatesLeadingAndTrailingSilence()
    try resumesAfterDroppedSilence()
    try chunkerEmitsChunkAfterTrailingSilence()
    try chunkerIgnoresAudioShorterThanMinimumSpeech()
    try chunkerSplitsAtMaxChunkDuration()
    try chunkerReportsStartAndDurationFromCumulativeFrames()
    try chunkerIncludesPreRollBeforeSpeechOnset()
    try chunkerFlushReturnsPendingSpeech()
    try chunkerCutsUtteranceDespiteElevatedNoiseFloor()
    try chunkerDetectsSpeechWhenSessionStartsMidSpeech()
    try parsesWhisperCliFullJson()
    try computesMeanTokenProbabilityConfidence()
    try sanitizesWhisperNonSpeechAnnotations()
    try mapsWhisperLanguageToSessionLanguageByPrefix()
    try fallsBackToLanguageFitnessForUnmatchedWhisperLanguage()
    try buildsWhisperCliArguments()
    try encodesWhisperEngineConfigLine()
    try encodesWhisperEngineAudioLineAsPcm16Base64()
    try buffersWhisperEngineOutputLinesAcrossChunks()
    try buildsTranscriptEventFromWhisperEngineOutput()
    try await terminatesHungWhisperProcessAfterTimeout()
    try await terminatesWhisperProcessOnTaskCancellation()
    try keepsMeaningfulTranscriptRules()
    try buildsTranscriptCandidateFromWhisperResult()
    try dropsNonSpeechWhisperResult()
    try writesInt16MonoWavReadableByAVAudioFile()
    try configuresScreenCaptureKitSpeakerStream()
    try configuresScreenCaptureKitSpeakerAudioFormat()
    try overlapsCandidatesSharingMostOfTheirRange()
    try prefersConfidentCandidate()
    try prefersScriptMatchingCandidate()
    try penalizesImplausiblyDenseTranscript()
    try combinesSequentialFinalsPerLanguage()
    try arbitratesGroupAcrossLanguages()
    try assignsWaveformBuckets()
    try clampsMixedSamples()
    try await arbiterSuppressesCrossLanguageInterimFlicker()
    try await arbiterPairsFinalsAcrossLanguages()
    try await arbiterFlushesUnpairedFinalAfterHold()
    try await arbiterDropsLateCounterpartFinalForFlushedUtterance()
    try await arbiterExtendsHoldWhileCounterpartVolatileIsActive()
    print("all swift helper tests passed")
  }

  static func parsesDetectLanguagesCommand() throws {
    let options = try CommandLineOptions.parse(["helper", "--detect-languages"])
    try expectEqual(options.command, .detectLanguages)
  }

  static func parsesInstallLanguageCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--install-language", "fr-FR"]).command,
      .installLanguage("fr-FR")
    )
  }

  static func parsesUninstallLanguageCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--uninstall-language", "fr-FR"]).command,
      .uninstallLanguage("fr-FR")
    )
  }

  static func parsesAiServerCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--ai-server"]).command,
      .aiServer
    )
  }

  static func parsesWaveformCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--waveform", "/tmp/mic.m4a"]).command,
      .waveform(path: "/tmp/mic.m4a", buckets: defaultWaveformBuckets)
    )
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--waveform", "/tmp/mic.m4a", "--buckets", "300"]).command,
      .waveform(path: "/tmp/mic.m4a", buckets: 300)
    )
  }

  static func parsesMixCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse([
        "helper",
        "--mix",
        "--input", "/tmp/mic.m4a",
        "--input", "/tmp/speaker.m4a",
        "--output", "/tmp/mixed.m4a"
      ]).command,
      .mix(inputs: ["/tmp/mic.m4a", "/tmp/speaker.m4a"], output: "/tmp/mixed.m4a")
    )

    do {
      _ = try CommandLineOptions.parse(["helper", "--mix", "--input", "/tmp/only-one.m4a"])
      throw TestFailure(message: "Expected mix parsing to fail without two inputs and an output")
    } catch is CommandLineOptionsError {
      // expected
    }
  }

  static func parsesMicStreamCommandWithLocales() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream",
      "mic",
      "--source-language",
      "en-US",
      "--target-language",
      "ja-JP"
    ])

    try expectEqual(options.command, .stream(.mic))
    try expectEqual(options.sourceLanguage, "en-US")
    try expectEqual(options.targetLanguage, "ja-JP")
  }

  static func parsesSpeakerStreamCommandWithSegmentDirectory() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream",
      "speaker",
      "--language",
      "en-US",
      "--language",
      "ja-JP",
      "--segment-directory",
      "/tmp/live-poly-trans"
    ])

    try expectEqual(options.command, .stream(.speaker))
    try expectEqual(options.languages, ["en-US", "ja-JP"])
    try expectEqual(options.segmentDirectory, "/tmp/live-poly-trans")
  }

  static func parsesStreamCommandWithRecordingFiles() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream",
      "mic",
      "--record-file",
      "/tmp/rec/mic.m4a",
      "--transcript-file",
      "/tmp/rec/mic.jsonl"
    ])

    try expectEqual(options.recordFile, "/tmp/rec/mic.m4a")
    try expectEqual(options.transcriptFile, "/tmp/rec/mic.jsonl")
  }

  static func parsesStreamCommandWithWhisperEngine() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream", "mic",
      "--transcription-engine", "whisper",
      "--whisper-model", "/models/ggml-large-v3-turbo.bin",
      "--whisper-cli", "/opt/homebrew/bin/whisper-cli"
    ])

    try expectEqual(options.transcriptionEngine, .whisper)
    try expectEqual(options.whisperModel, "/models/ggml-large-v3-turbo.bin")
    try expectEqual(options.whisperCli, "/opt/homebrew/bin/whisper-cli")
  }

  static func parsesStreamCommandWithWhisperEngineSidecar() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream", "mic",
      "--transcription-engine", "whisper",
      "--whisper-model", "/models/ggml-large-v3-turbo.bin",
      "--whisper-engine", "/app/lpt-whisper-engine"
    ])

    try expectEqual(options.transcriptionEngine, .whisper)
    try expectEqual(options.whisperModel, "/models/ggml-large-v3-turbo.bin")
    try expectEqual(options.whisperEngine, "/app/lpt-whisper-engine")
  }

  static func defaultsToBuiltinTranscriptionEngine() throws {
    let options = try CommandLineOptions.parse(["helper", "--stream", "mic"])

    try expectEqual(options.transcriptionEngine, .builtin)
    try expectEqual(options.whisperModel, nil)
    try expectEqual(options.whisperCli, nil)
  }

  static func rejectsUnknownTranscriptionEngine() throws {
    do {
      _ = try CommandLineOptions.parse([
        "helper", "--stream", "mic", "--transcription-engine", "parakeet"
      ])
      throw TestFailure(message: "Expected parsing to reject an unknown transcription engine")
    } catch is CommandLineOptionsError {
      // expected
    }
  }

  static func rejectsWhisperEngineWithoutModelAndCliPaths() throws {
    let incompleteArguments: [[String]] = [
      ["helper", "--stream", "mic", "--transcription-engine", "whisper"],
      [
        "helper", "--stream", "mic", "--transcription-engine", "whisper",
        "--whisper-model", "/models/ggml-large-v3-turbo.bin"
      ],
      [
        "helper", "--stream", "mic", "--transcription-engine", "whisper",
        "--whisper-cli", "/opt/homebrew/bin/whisper-cli"
      ]
    ]

    for arguments in incompleteArguments {
      do {
        _ = try CommandLineOptions.parse(arguments)
        throw TestFailure(message: "Expected whisper engine to require model and cli paths: \(arguments)")
      } catch is CommandLineOptionsError {
        // expected
      }
    }
  }

  // MARK: - UtteranceChunker
  // 16 kHz mono; test buffers are 4096 frames = 256 ms, mirroring the
  // capture tap granularity.

  static func makeChunker(maxChunkMs: Int = 28_000) -> UtteranceChunker {
    UtteranceChunker(
      sampleRate: 16_000,
      silenceThresholdRMS: 0.01,
      minSpeechMs: 300,
      trailingSilenceMs: 800,
      maxChunkMs: maxChunkMs,
      preRollMs: 200
    )
  }

  static let chunkerSpeechBuffer = [Float](repeating: 0.1, count: 4096)
  static let chunkerSilentBuffer = [Float](repeating: 0, count: 4096)

  static func chunkerEmitsChunkAfterTrailingSilence() throws {
    let chunker = makeChunker()
    var chunks: [UtteranceChunk] = []

    for _ in 0..<4 {
      chunks += chunker.consume(chunkerSpeechBuffer)
    }
    try expectEqual(chunks.count, 0)

    for _ in 0..<4 {
      chunks += chunker.consume(chunkerSilentBuffer)
    }

    try expectEqual(chunks.count, 1)
    try expectEqual(chunks[0].startMs, 0)
    try expectEqual(chunks[0].durationMs, 2048)
    try expectEqual(chunks[0].samples.count, 8 * 4096)
  }

  static func chunkerIgnoresAudioShorterThanMinimumSpeech() throws {
    let chunker = makeChunker()
    var chunks: [UtteranceChunk] = []

    chunks += chunker.consume(chunkerSpeechBuffer)
    for _ in 0..<4 {
      chunks += chunker.consume(chunkerSilentBuffer)
    }

    try expectEqual(chunks.count, 0)
  }

  static func chunkerSplitsAtMaxChunkDuration() throws {
    let chunker = makeChunker(maxChunkMs: 1_000)
    var chunks: [UtteranceChunk] = []

    for _ in 0..<6 {
      chunks += chunker.consume(chunkerSpeechBuffer)
    }
    for _ in 0..<4 {
      chunks += chunker.consume(chunkerSilentBuffer)
    }

    try expectEqual(chunks.count, 2)
    try expectEqual(chunks[0].startMs, 0)
    try expectEqual(chunks[0].durationMs, 1024)
    // The second chunk starts seamlessly where the first was cut and hits
    // the cap again (2 speech + 2 silent buffers) before the 800 ms
    // trailing-silence rule can fire.
    try expectEqual(chunks[1].startMs, 1024)
    try expectEqual(chunks[1].durationMs, 1024)
  }

  static func chunkerReportsStartAndDurationFromCumulativeFrames() throws {
    let chunker = makeChunker()
    var chunks: [UtteranceChunk] = []

    for _ in 0..<4 {
      chunks += chunker.consume(chunkerSilentBuffer)
    }
    for _ in 0..<2 {
      chunks += chunker.consume(chunkerSpeechBuffer)
    }
    for _ in 0..<4 {
      chunks += chunker.consume(chunkerSilentBuffer)
    }

    try expectEqual(chunks.count, 1)
    // Speech starts at 1024 ms into the stream; the chunk opens 200 ms of
    // pre-roll earlier.
    try expectEqual(chunks[0].startMs, 824)
    // Pre-roll (200 ms) + 2 speech buffers (512 ms) + trailing silence up to
    // the cut (1024 ms).
    try expectEqual(chunks[0].durationMs, 1736)
  }

  static func chunkerIncludesPreRollBeforeSpeechOnset() throws {
    let chunker = makeChunker()
    _ = chunker.consume(chunkerSilentBuffer)
    _ = chunker.consume(chunkerSpeechBuffer)
    _ = chunker.consume(chunkerSpeechBuffer)

    guard let chunk = chunker.flush() else {
      throw TestFailure(message: "Expected flush to return the pending utterance")
    }

    // 200 ms of pre-roll (3200 frames) precede the 2 speech buffers.
    try expectEqual(chunk.samples.count, 3_200 + 2 * 4096)
    try expectEqual(chunk.samples.prefix(3_200).allSatisfy { $0 == 0 }, true)
  }

  static func chunkerFlushReturnsPendingSpeech() throws {
    let chunker = makeChunker()
    _ = chunker.consume(chunkerSpeechBuffer)
    _ = chunker.consume(chunkerSpeechBuffer)

    guard let chunk = chunker.flush() else {
      throw TestFailure(message: "Expected flush to return the pending utterance")
    }

    try expectEqual(chunk.startMs, 0)
    try expectEqual(chunk.durationMs, 512)
    try expectEqual(chunker.flush(), nil)

    // Pending audio below the minimum speech duration is noise, not an
    // utterance; flushing it must not produce a chunk.
    _ = chunker.consume(chunkerSpeechBuffer)
    try expectEqual(chunker.flush(), nil)
  }

  // Loopback audio from a video call carries comfort noise well above the
  // speaker stream's 0.001 absolute threshold. The chunker must learn that
  // floor and still cut utterances at pauses instead of running every chunk
  // to the 28 s ceiling.
  static func chunkerCutsUtteranceDespiteElevatedNoiseFloor() throws {
    let chunker = UtteranceChunker(
      sampleRate: 16_000,
      silenceThresholdRMS: 0.001,
      minSpeechMs: 300,
      trailingSilenceMs: 800,
      maxChunkMs: 28_000,
      preRollMs: 200
    )
    let comfortNoise = [Float](repeating: 0.005, count: 4096)
    let speech = [Float](repeating: 0.1, count: 4096)
    var chunks: [UtteranceChunk] = []

    for _ in 0..<2 {
      chunks += chunker.consume(comfortNoise)
    }
    for _ in 0..<4 {
      chunks += chunker.consume(speech)
    }
    for _ in 0..<4 {
      chunks += chunker.consume(comfortNoise)
    }

    try expectEqual(chunks.count, 1)
    // Speech starts at buffer 2 (512 ms); the chunk opens 200 ms earlier
    // with pre-roll.
    try expectEqual(chunks[0].startMs, 312)
    try expectEqual(chunks[0].durationMs, 2_248)
  }

  // A session that starts mid-speech must not learn the speech level as the
  // noise floor and classify the whole utterance as silence.
  static func chunkerDetectsSpeechWhenSessionStartsMidSpeech() throws {
    let chunker = UtteranceChunker(
      sampleRate: 16_000,
      silenceThresholdRMS: 0.001,
      minSpeechMs: 300,
      trailingSilenceMs: 800,
      maxChunkMs: 28_000,
      preRollMs: 200
    )
    let speech = [Float](repeating: 0.1, count: 4096)
    var chunks: [UtteranceChunk] = []

    for _ in 0..<4 {
      chunks += chunker.consume(speech)
    }
    for _ in 0..<4 {
      chunks += chunker.consume(chunkerSilentBuffer)
    }

    try expectEqual(chunks.count, 1)
    try expectEqual(chunks[0].startMs, 0)
    try expectEqual(chunks[0].durationMs, 2_048)
  }

  // MARK: - Whisper result parsing
  // Fixture mirrors whisper-cli --output-json-full: result.language holds the
  // detected code, transcription carries segments with per-token
  // probabilities. Extra fields the parser ignores (systeminfo, params,
  // token ids/timestamps) are included to keep the fixture honest.

  static let whisperFixtureJson = """
  {
    "systeminfo": "AVX = 0 | NEON = 1",
    "model": {"type": "large v3"},
    "params": {"model": "/models/ggml-large-v3-turbo.bin", "language": "auto"},
    "result": {"language": "ja"},
    "transcription": [
      {
        "timestamps": {"from": "00:00:00,000", "to": "00:00:02,000"},
        "offsets": {"from": 0, "to": 2000},
        "text": "こんにちは、",
        "tokens": [
          {"text": "[_BEG_]", "id": 50365, "p": 0.99, "t_dtw": -1},
          {"text": "こんにちは", "id": 38088, "p": 0.9, "t_dtw": -1},
          {"text": "、", "id": 1231, "p": 0.7, "t_dtw": -1}
        ]
      },
      {
        "timestamps": {"from": "00:00:02,000", "to": "00:00:04,420"},
        "offsets": {"from": 2000, "to": 4420},
        "text": "会議を始めましょう。",
        "tokens": [
          {"text": "会議", "id": 12949, "p": 0.8, "t_dtw": -1}
        ]
      }
    ]
  }
  """

  static func parsesWhisperCliFullJson() throws {
    let result = try parsedWhisperResult(Data(whisperFixtureJson.utf8))

    try expectEqual(result.result.language, "ja")
    try expectEqual(result.transcription.count, 2)
    try expectEqual(result.transcription[0].text, "こんにちは、")
    try expectEqual(result.transcription[1].offsets.from, 2000)
    try expectEqual(result.transcription[1].offsets.to, 4420)
    try expectEqual(result.transcription[0].tokens?.count, 3)
  }

  static func computesMeanTokenProbabilityConfidence() throws {
    let result = try parsedWhisperResult(Data(whisperFixtureJson.utf8))

    // Special tokens like [_BEG_] carry decoder bookkeeping, not speech;
    // they are excluded from the mean: (0.9 + 0.7 + 0.8) / 3.
    guard let confidence = whisperConfidence(result) else {
      throw TestFailure(message: "Expected a confidence from token probabilities")
    }
    try expectEqual((confidence * 100).rounded() / 100, 0.8)
  }

  static func sanitizesWhisperNonSpeechAnnotations() throws {
    try expectEqual(sanitizedWhisperText("[BLANK_AUDIO]"), "")
    try expectEqual(sanitizedWhisperText(" (upbeat music) "), "")
    try expectEqual(sanitizedWhisperText("♪♪"), "")
    try expectEqual(sanitizedWhisperText("ご視聴ありがとうございました"), "")
    try expectEqual(sanitizedWhisperText("ご視聴ありがとうございました。"), "")
    try expectEqual(sanitizedWhisperText(" Hello there. [BLANK_AUDIO]"), "Hello there.")
    try expectEqual(sanitizedWhisperText("こんにちは、会議を始めましょう。"), "こんにちは、会議を始めましょう。")
  }

  static func mapsWhisperLanguageToSessionLanguageByPrefix() throws {
    try expectEqual(
      sessionLanguage(forWhisperLanguage: "ja", sourceLanguage: "ja-JP", targetLanguage: "en-US", text: "こんにちは"),
      "ja-JP"
    )
    try expectEqual(
      sessionLanguage(forWhisperLanguage: "en", sourceLanguage: "ja-JP", targetLanguage: "en-US", text: "hello"),
      "en-US"
    )
  }

  static func fallsBackToLanguageFitnessForUnmatchedWhisperLanguage() throws {
    try expectEqual(
      sessionLanguage(forWhisperLanguage: "zh", sourceLanguage: "ja-JP", targetLanguage: "en-US", text: "こんにちは、会議"),
      "ja-JP"
    )
    try expectEqual(
      sessionLanguage(forWhisperLanguage: "ko", sourceLanguage: "ja-JP", targetLanguage: "en-US", text: "hello there"),
      "en-US"
    )
  }

  static func buildsWhisperCliArguments() throws {
    try expectEqual(
      whisperCliArguments(
        modelPath: "/models/ggml-large-v3-turbo.bin",
        audioPath: "/tmp/chunk-0.wav",
        outputBase: "/tmp/chunk-0"
      ),
      [
        "-m", "/models/ggml-large-v3-turbo.bin",
        "-f", "/tmp/chunk-0.wav",
        "-l", "auto",
        "--output-json-full",
        "-of", "/tmp/chunk-0",
        "--no-prints"
      ]
    )
  }

  static func encodesWhisperEngineConfigLine() throws {
    try expectEqual(
      try whisperEngineConfigLine(
        modelPath: "/models/ggml-large-v3-turbo.bin",
        language: "auto",
        sampleRate: 16_000
      ),
      #"{"language":"auto","modelPath":"\/models\/ggml-large-v3-turbo.bin","sampleRate":16000,"type":"config"}"#
    )
  }

  static func encodesWhisperEngineAudioLineAsPcm16Base64() throws {
    let line = try whisperEngineAudioLine(
      stream: .mic,
      seq: 7,
      timestampMs: 125,
      samples: [0, 1, -1]
    )
    let data = Data(line.utf8)
    let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
    let payload = try expectValue(object?["pcm16Base64"] as? String, "missing audio payload")
    let decoded = try expectValue(Data(base64Encoded: payload), "invalid base64 payload")

    try expectEqual(object?["type"] as? String, "audio")
    try expectEqual(object?["stream"] as? String, "mic")
    try expectEqual(object?["seq"] as? Int, 7)
    try expectEqual(object?["timestampMs"] as? Int, 125)
    try expectEqual(Array(decoded), [0, 0, 255, 127, 1, 128])
  }

  static func buffersWhisperEngineOutputLinesAcrossChunks() throws {
    let accumulator = JsonLineAccumulator()

    try expectEqual(accumulator.consume(Data(#"{"type":"sta"#.utf8)), [])
    try expectEqual(
      accumulator.consume(Data(#"tus","state":"ready"}"#.utf8)),
      []
    )
    try expectEqual(
      accumulator.consume(Data("\n{\"type\":\"status\",\"state\":\"done\"}\n".utf8)),
      [
        #"{"type":"status","state":"ready"}"#,
        #"{"type":"status","state":"done"}"#
      ]
    )
  }

  static func buildsTranscriptEventFromWhisperEngineOutput() throws {
    let output = WhisperEngineOutputLine(
      type: "transcript",
      state: nil,
      message: nil,
      fatal: nil,
      stream: "mic",
      segmentId: "mic-rolling",
      startMs: 250,
      durationMs: 1500,
      text: "こんにちは",
      isFinal: false,
      language: "ja",
      confidence: 0.82
    )
    let event = try expectValue(
      transcriptEvent(
        from: output,
        stream: .mic,
        sourceLanguage: "ja-JP",
        targetLanguage: "en-US",
        timestamp: Date(timeIntervalSince1970: 0)
      ),
      "expected transcript event"
    )

    try expectEqual(event.lang, "ja-JP")
    try expectEqual(event.text, "こんにちは")
    try expectEqual(event.isFinal, false)
    try expectEqual(event.segmentId, "mic-rolling")
    try expectEqual(event.confidence, 0.82)
  }

  // A wedged whisper-cli (Metal hang, stalled volume) must not block the
  // transcription queue forever: the runner kills it after the timeout and
  // surfaces a normal error so the session moves on to the next chunk.
  static func terminatesHungWhisperProcessAfterTimeout() async throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/sleep")
    process.arguments = ["30"]

    let startedAt = Date()
    let status = try await runUntilExit(process, timeoutSeconds: 0.2)

    try expectEqual(status == 0, false)
    try expectEqual(Date().timeIntervalSince(startedAt) < 5, true)
  }

  static func terminatesWhisperProcessOnTaskCancellation() async throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/sleep")
    process.arguments = ["30"]

    let startedAt = Date()
    let task = Task {
      try await runUntilExit(process, timeoutSeconds: 60)
    }
    try await Task.sleep(nanoseconds: 100_000_000)
    task.cancel()
    let status = try? await task.value

    try expectEqual(status == nil || status != 0, true)
    try expectEqual(Date().timeIntervalSince(startedAt) < 5, true)
    try expectEqual(process.isRunning, false)
  }

  static func keepsMeaningfulTranscriptRules() throws {
    try expectEqual(isMeaningfulTranscript("ok", isFinal: true), true)
    try expectEqual(isMeaningfulTranscript("ok", isFinal: false), false)
    try expectEqual(isMeaningfulTranscript("okay", isFinal: false), true)
    try expectEqual(isMeaningfulTranscript("...", isFinal: true), false)
    try expectEqual(isMeaningfulTranscript("", isFinal: true), false)
  }

  static func buildsTranscriptCandidateFromWhisperResult() throws {
    let result = try parsedWhisperResult(Data(whisperFixtureJson.utf8))
    guard
      let candidate = whisperTranscriptCandidate(
        result: result,
        chunkStartMs: 5_000,
        chunkDurationMs: 4_420,
        sourceLanguage: "ja-JP",
        targetLanguage: "en-US"
      )
    else {
      throw TestFailure(message: "Expected a candidate from a meaningful whisper result")
    }

    try expectEqual(candidate.language, "ja-JP")
    try expectEqual(candidate.text, "こんにちは、会議を始めましょう。")
    try expectEqual(candidate.isFinal, true)
    try expectEqual(candidate.startMs, 5_000)
    try expectEqual(candidate.durationMs, 4_420)
    try expectEqual(candidate.segmentId, "5000-4420")
    try expectEqual(candidate.detectedLanguage, "ja")
  }

  static func dropsNonSpeechWhisperResult() throws {
    let blankJson = """
    {
      "result": {"language": "en"},
      "transcription": [
        {
          "timestamps": {"from": "00:00:00,000", "to": "00:00:01,000"},
          "offsets": {"from": 0, "to": 1000},
          "text": " [BLANK_AUDIO]",
          "tokens": []
        }
      ]
    }
    """
    let result = try parsedWhisperResult(Data(blankJson.utf8))
    let candidate = whisperTranscriptCandidate(
      result: result,
      chunkStartMs: 0,
      chunkDurationMs: 1_000,
      sourceLanguage: "ja-JP",
      targetLanguage: "en-US"
    )

    try expectEqual(candidate, nil)
  }

  static func writesInt16MonoWavReadableByAVAudioFile() throws {
    let url = FileManager.default.temporaryDirectory
      .appendingPathComponent("lpt-wav-test-\(UUID().uuidString).wav")
    defer { try? FileManager.default.removeItem(at: url) }

    let samples = (0..<16_000).map { Float(sin(Double($0) * 0.1)) * 0.5 }
    try writeInt16MonoWav(samples: samples, sampleRate: 16_000, to: url)

    let file = try AVAudioFile(forReading: url)
    try expectEqual(file.length, 16_000)
    try expectEqual(file.fileFormat.sampleRate, 16_000)
    try expectEqual(file.fileFormat.channelCount, 1)
  }

  static func expectEqual<T: Equatable>(_ actual: T, _ expected: T) throws {
    if actual != expected {
      throw TestFailure(message: "Expected \(expected), got \(actual)")
    }
  }

  static func expectValue<T>(_ value: T?, _ message: String) throws -> T {
    guard let value else {
      throw TestFailure(message: message)
    }

    return value
  }

  static func parsesTrimCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse([
        "helper",
        "--trim",
        "--input", "/tmp/mic.m4a",
        "--output", "/tmp/mic-trimmed.m4a",
        "--start-ms", "5000",
        "--end-ms", "12000"
      ]).command,
      .trim(input: "/tmp/mic.m4a", output: "/tmp/mic-trimmed.m4a", startMs: 5000, endMs: 12000)
    )

    do {
      _ = try CommandLineOptions.parse(["helper", "--trim", "--input", "/tmp/mic.m4a"])
      throw TestFailure(message: "expected trim without range to fail")
    } catch is CommandLineOptionsError {}
  }

  static func trimsAudioFileToRange() throws {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("lpt-trim-test-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }

    // 2 s of a 440 Hz tone at 16 kHz.
    let sampleRate = 16_000.0
    let samples = (0..<Int(sampleRate * 2)).map { index in
      Float(sin(2 * Double.pi * 440 * Double(index) / sampleRate)) * 0.5
    }
    let inputUrl = directory.appendingPathComponent("input.wav")
    try writeInt16MonoWav(samples: samples, sampleRate: sampleRate, to: inputUrl)

    let outputUrl = directory.appendingPathComponent("trimmed.m4a")
    try trimAudioFile(
      inputPath: inputUrl.path,
      outputPath: outputUrl.path,
      startMs: 500,
      endMs: 1500
    )

    let output = try AVAudioFile(forReading: outputUrl)
    let durationMs =
      Double(output.length) / output.processingFormat.sampleRate * 1000
    guard abs(durationMs - 1000) < 120 else {
      throw TestFailure(message: "trimmed duration \(durationMs)ms, expected ~1000ms")
    }
  }

  /// Recordings are AAC .m4a, and AVAudioFile signals their end by throwing
  /// instead of returning an empty buffer, so the whole waveform was lost on
  /// the very last read.
  static func computesWaveformForAacFile() throws {
    let directory = try makeTemporaryDirectory(prefix: "lpt-waveform-test")
    defer { try? FileManager.default.removeItem(at: directory) }

    let inputUrl = directory.appendingPathComponent("input.wav")
    try writeInt16MonoWav(samples: toneSamples(seconds: 2), sampleRate: 16_000, to: inputUrl)
    let aacUrl = directory.appendingPathComponent("input.m4a")
    try trimAudioFile(
      inputPath: inputUrl.path,
      outputPath: aacUrl.path,
      startMs: 0,
      endMs: 2_000
    )

    let waveform = try computeWaveform(path: aacUrl.path, buckets: 100)
    try expectEqual(waveform.peaks.count, 100)
    guard abs(waveform.durationMs - 2_000) < 200 else {
      throw TestFailure(message: "waveform duration \(waveform.durationMs)ms, expected ~2000ms")
    }
    guard waveform.peaks.allSatisfy({ $0 > 0.1 }) else {
      throw TestFailure(message: "expected every bucket of a continuous tone to have a peak")
    }
  }

  static func mixesAacFiles() throws {
    let directory = try makeTemporaryDirectory(prefix: "lpt-mix-test")
    defer { try? FileManager.default.removeItem(at: directory) }

    let wavUrl = directory.appendingPathComponent("input.wav")
    try writeInt16MonoWav(samples: toneSamples(seconds: 1), sampleRate: 16_000, to: wavUrl)
    let inputs = try ["a.m4a", "b.m4a"].map { name -> String in
      let url = directory.appendingPathComponent(name)
      try trimAudioFile(inputPath: wavUrl.path, outputPath: url.path, startMs: 0, endMs: 1_000)
      return url.path
    }

    let outputUrl = directory.appendingPathComponent("mixed.m4a")
    try mixAudioFiles(inputs: inputs, outputPath: outputUrl.path)

    let output = try AVAudioFile(forReading: outputUrl)
    let durationMs = Double(output.length) / output.processingFormat.sampleRate * 1000
    guard abs(durationMs - 1_000) < 200 else {
      throw TestFailure(message: "mixed duration \(durationMs)ms, expected ~1000ms")
    }
  }

  static func makeTemporaryDirectory(prefix: String) throws -> URL {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("\(prefix)-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    return directory
  }

  /// A 440 Hz tone at 16 kHz: loud in every bucket, so a dropped tail shows up.
  static func toneSamples(seconds: Int) -> [Float] {
    (0..<(16_000 * seconds)).map { index in
      Float(sin(2 * Double.pi * 440 * Double(index) / 16_000)) * 0.5
    }
  }

  static func treatsEmptyLanguageArgumentsAsUnset() throws {
    // The app sends --target-language "" for 翻訳しない; an empty identifier
    // must not reach the translator as a real language.
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream", "mic",
      "--source-language", "ja-JP",
      "--target-language", ""
    ])

    try expectEqual(options.sourceLanguage, "ja-JP")
    try expectEqual(options.targetLanguage, nil)
  }

  static func parsesStartRecordingControlLine() throws {
    try expectEqual(
      parseHelperControlLine(#"{"cmd":"start-recording","dir":"/tmp/rec-1"}"#),
      .startRecording(directory: "/tmp/rec-1", includeAudio: true)
    )
    try expectEqual(
      parseHelperControlLine(#"{"cmd":"start-recording","dir":"/tmp/rec-1","audio":false}"#),
      .startRecording(directory: "/tmp/rec-1", includeAudio: false)
    )
    try expectEqual(
      parseHelperControlLine(#"{"cmd":"start-recording","dir":"/tmp/rec-1","audio":true}"#),
      .startRecording(directory: "/tmp/rec-1", includeAudio: true)
    )
  }

  static func parsesStopRecordingControlLine() throws {
    try expectEqual(parseHelperControlLine(#"{"cmd":"stop-recording"}"#), .stopRecording)
  }

  static func ignoresMalformedControlLines() throws {
    try expectEqual(parseHelperControlLine(""), nil)
    try expectEqual(parseHelperControlLine("   \n"), nil)
    try expectEqual(parseHelperControlLine("not json"), nil)
    try expectEqual(parseHelperControlLine(#"{"cmd":"unknown"}"#), nil)
    try expectEqual(parseHelperControlLine(#"{"cmd":"start-recording"}"#), nil)
    try expectEqual(parseHelperControlLine(#"{"cmd":"start-recording","dir":""}"#), nil)
    try expectEqual(parseHelperControlLine(#"["cmd"]"#), nil)
  }

  static func choosesNonClobberingRecordingFilePath() throws {
    try expectEqual(
      nextRecordingFilePath(directory: "/rec", stream: .mic, fileExists: { _ in false }),
      "/rec/mic.m4a"
    )
    try expectEqual(
      nextRecordingFilePath(
        directory: "/rec",
        stream: .speaker,
        fileExists: { $0 == "/rec/speaker.m4a" }
      ),
      "/rec/speaker-2.m4a"
    )
    try expectEqual(
      nextRecordingFilePath(
        directory: "/rec",
        stream: .mic,
        fileExists: { $0 == "/rec/mic.m4a" || $0 == "/rec/mic-2.m4a" }
      ),
      "/rec/mic-3.m4a"
    )
  }

  static func readsValueAfterFlag() throws {
    try expectEqual(value(after: "--stream", in: ["helper", "--stream", "mic"]), "mic")
    try expectEqual(value(after: "--missing", in: ["helper", "--stream", "mic"]), nil)
  }

  static func readsRepeatedValuesAfterFlag() throws {
    try expectEqual(
      values(after: "--language", in: ["helper", "--language", "en-US", "--language", "ja-JP"]),
      ["en-US", "ja-JP"]
    )
  }

  static func formatsIso8601Timestamp() throws {
    try expectEqual(iso8601Timestamp(Date(timeIntervalSince1970: 0)), "1970-01-01T00:00:00Z")
  }

  static func formatsLanguageInfoWithBcp47Identifier() throws {
    let language = languageInfo(from: Locale(identifier: "ja_JP"))
    try expectEqual(language.id, "ja-JP")
    try expectEqual(language.label, "Japanese (Japan)")
  }

  static func encodesJsonLine() throws {
    let line = try jsonLine(for: LanguageInfo(id: "en-US", label: "English"))
    try expectEqual(line, #"{"id":"en-US","label":"English"}"#)
  }

  static func formatsTranscriptEvent() throws {
    let event = transcriptEvent(
      stream: .mic,
      language: "en-US",
      text: "hello",
      translation: nil,
      isFinal: true,
      timestamp: Date(timeIntervalSince1970: 0),
      segmentId: "0-1000",
      confidence: 0.75,
      spans: [
        TranscriptSpan(text: "hello", confidence: 0.75, startMs: 0, endMs: 500)
      ]
    )

    try expectEqual(event.type, "transcript")
    try expectEqual(event.stream, "mic")
    try expectEqual(event.speakerId, "self")
    try expectEqual(event.speakerLabel, "Speaker A")
    try expectEqual(event.time, "1970-01-01T00:00:00Z")
    try expectEqual(event.timestamp, "1970-01-01T00:00:00Z")
    try expectEqual(event.confidence, 0.75)
    try expectEqual(event.detectedLang, "en")
    try expectEqual(event.spans, [TranscriptSpan(text: "hello", confidence: 0.75, startMs: 0, endMs: 500)])
  }

  static func formatsTranslationEvent() throws {
    let event = translationEvent(
      stream: .speaker,
      segmentId: "1200-800",
      language: "en-US",
      targetLanguage: "ja-JP",
      translation: "こんにちは",
      timestamp: Date(timeIntervalSince1970: 0)
    )

    try expectEqual(event.type, "translation")
    try expectEqual(event.stream, "speaker")
    try expectEqual(event.segmentId, "1200-800")
    try expectEqual(event.targetLanguage, "ja-JP")
    try expectEqual(event.trans, "こんにちは")
    try expectEqual(event.timestamp, "1970-01-01T00:00:00Z")
  }

  static func extractsTranscriptSpansFromSpeechAttributes() throws {
    guard #available(macOS 26.0, *) else {
      return
    }

    var text = AttributedString("hello")
    text[text.startIndex..<text.endIndex][AttributeScopes.SpeechAttributes.ConfidenceAttribute.self] = 0.75
    text[text.startIndex..<text.endIndex][AttributeScopes.SpeechAttributes.TimeRangeAttribute.self] = CMTimeRange(
      start: CMTime(value: 100, timescale: 1000),
      duration: CMTime(value: 500, timescale: 1000)
    )

    let spans = transcriptSpans(from: text)

    try expectEqual(spans, [TranscriptSpan(text: "hello", confidence: 0.75, startMs: 100, endMs: 600)])
    try expectEqual(transcriptConfidence(spans: spans), 0.75)
  }

  static func detectsTranscriptLanguage() throws {
    let detection = detectedTranscriptLanguage("今日はよろしくお願いします")
    try expectEqual(detection?.language, "ja")
  }

  static func buildsMeetingAiPrompts() throws {
    let transcript = "[2026-06-23T10:00:00Z] Speaker A / ja-JP: 次のリリースを決めます"

    let japaneseSummaryPrompt = meetingSummaryPrompt(
      transcript: transcript,
      previousSummary: nil,
      responseLanguage: "ja-JP"
    )

    if !japaneseSummaryPrompt.contains("Speaker A") {
      throw TestFailure(message: "Expected summary prompt to include transcript context")
    }

    if !japaneseSummaryPrompt.contains("Write the entire response in Japanese (ja-JP).") {
      throw TestFailure(message: "Expected summary prompt to require Japanese for Japanese main language")
    }

    if !japaneseSummaryPrompt.contains("Do not write the summary in English.") {
      throw TestFailure(message: "Expected summary prompt to forbid English when main language is Japanese")
    }

    if !japaneseSummaryPrompt.contains("概要") {
      throw TestFailure(message: "Expected Japanese section labels for Japanese main language")
    }

    let questionsPrompt = suggestedQuestionsPrompt(transcript: transcript, responseLanguage: "ja-JP")
    if !questionsPrompt.contains("Japanese (ja-JP)") {
      throw TestFailure(message: "Expected questions prompt to carry the response language")
    }
  }

  static func buildsRollingSummaryPrompt() throws {
    let prompt = meetingSummaryPrompt(
      transcript: "new lines",
      previousSummary: "- 概要: これまでの要約",
      responseLanguage: "ja-JP"
    )

    if !prompt.contains("Existing summary:") || !prompt.contains("これまでの要約") {
      throw TestFailure(message: "Expected rolling summary prompt to include the previous summary")
    }

    if !prompt.contains("New transcript portion:") {
      throw TestFailure(message: "Expected rolling summary prompt to label the new transcript portion")
    }
  }

  static func buildsChatPromptWithHistory() throws {
    let prompt = meetingQuestionPrompt(
      question: "リリース日は?",
      transcript: "transcript body",
      history: [AiChatTurn(question: "何を決めた?", answer: "リリース計画です")],
      responseLanguage: "ja-JP"
    )

    if !prompt.contains("リリース日は?") || !prompt.contains("transcript body") {
      throw TestFailure(message: "Expected answer prompt to include the question and transcript")
    }

    if !prompt.contains("Q: 何を決めた?") || !prompt.contains("A: リリース計画です") {
      throw TestFailure(message: "Expected answer prompt to include chat history")
    }

    if !prompt.contains("Japanese (ja-JP)") {
      throw TestFailure(message: "Expected answer prompt to carry the response language")
    }
  }

  static func decodesAiServerRequest() throws {
    let json = #"{"id":"7","command":"ask","question":"when?","language":"ja-JP","transcript":"body","history":[{"question":"q","answer":"a"}]}"#
    let request = try JSONDecoder().decode(AiServerRequest.self, from: Data(json.utf8))

    try expectEqual(request.id, "7")
    try expectEqual(request.command, "ask")
    try expectEqual(request.question, "when?")
    try expectEqual(request.history, [AiChatTurn(question: "q", answer: "a")])
  }

  static func answersEmptyTranscriptWithoutModel() async throws {
    let response = await aiServerResponse(
      for: AiServerRequest(id: "1", command: "summary", language: "ja-JP", transcript: "  ")
    )

    try expectEqual(response.ok, true)
    try expectEqual(response.response, "まだ確定済みの文字起こしがありません。")
  }

  static func requestsTranscriptConfidenceAttributes() throws {
    guard #available(macOS 26.0, *) else {
      return
    }

    let options = transcriptAttributeOptions()

    if !options.contains(.transcriptionConfidence) {
      throw TestFailure(message: "Expected transcript attributes to request confidence")
    }

    if !options.contains(.audioTimeRange) {
      throw TestFailure(message: "Expected transcript attributes to request audio time ranges")
    }
  }

  static func labelsSpeakerStreamAsSystemAudioSpeaker() throws {
    try expectEqual(speakerIdentifier(for: .speaker), "system-audio")
    try expectEqual(speakerLabel(for: .speaker), "Speaker B")
  }

  static func describesScreenCapturePermissionRecovery() throws {
    if !screenCapturePermissionRecoveryMessage.contains("Screen & System Audio Recording") {
      throw TestFailure(message: "Expected screen capture recovery message to name the settings pane")
    }

    if !screenCapturePermissionRecoveryMessage.contains("LivePolyTransHelper") {
      throw TestFailure(message: "Expected screen capture recovery message to name the helper app")
    }
  }

  static func parsesCheckPermissionsCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--check-permissions"]).command,
      .checkPermissions
    )
  }

  static func parsesRequestPermissionCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--request-permission", "microphone"]).command,
      .requestPermission(.microphone)
    )
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--request-permission", "screen-recording"]).command,
      .requestPermission(.screenRecording)
    )
  }

  static func rejectsUnknownPermissionKind() throws {
    do {
      _ = try CommandLineOptions.parse(["helper", "--request-permission", "camera"])
      throw TestFailure(message: "Expected an unknown permission kind to be rejected")
    } catch let error as CommandLineOptionsError {
      try expectEqual(error, .unknownPermissionKind("camera"))
    }
  }

  static func mapsRecordPermissionOntoPermissionState() throws {
    try expectEqual(permissionState(forRecordPermission: .granted), .granted)
    try expectEqual(permissionState(forRecordPermission: .denied), .denied)
    try expectEqual(permissionState(forRecordPermission: .undetermined), .notDetermined)
  }

  /// CGPreflightScreenCaptureAccess only answers yes/no: a "no" cannot tell
  /// a first launch apart from a refusal, so it must not be reported as a
  /// denial the user has to go undo in System Settings.
  static func mapsScreenCapturePreflightOntoPermissionState() throws {
    try expectEqual(permissionState(forScreenCaptureGranted: true), .granted)
    try expectEqual(permissionState(forScreenCaptureGranted: false), .notDetermined)
  }

  static func encodesPermissionStatusPayload() throws {
    let payload = PermissionStatusPayload(microphone: .granted, screenRecording: .notDetermined)
    try expectEqual(
      try jsonLine(for: payload),
      #"{"microphone":"granted","screenRecording":"notDetermined"}"#
    )
  }

  static func calculatesAudioFrameLength() throws {
    guard
      let format = AVAudioFormat(
        commonFormat: .pcmFormatFloat32,
        sampleRate: 48_000,
        channels: 1,
        interleaved: false
      ),
      let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 960)
    else {
      throw TestFailure(message: "Could not create test audio buffer")
    }

    buffer.frameLength = 960
    try expectEqual(audioFrameLength(UnsafePointer(buffer.audioBufferList), format: format), 960)
  }

  static func gatesLeadingAndTrailingSilence() throws {
    let gate = AudioSilenceGate(silenceThresholdRMS: 0.0001, maxTrailingSilentFrames: 2_000)
    let silence = AudioSignalLevel(sampleCount: 960, rms: 0, peak: 0)
    let speech = AudioSignalLevel(sampleCount: 960, rms: 0.01, peak: 0.05)

    try expectEqual(gate.shouldEmit(level: silence, frameLength: 960), false)
    try expectEqual(gate.shouldEmit(level: speech, frameLength: 960), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 1_000), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 1_000), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 1), false)
  }

  static func resumesAfterDroppedSilence() throws {
    let gate = AudioSilenceGate(silenceThresholdRMS: 0.0001, maxTrailingSilentFrames: 1)
    let silence = AudioSignalLevel(sampleCount: 960, rms: 0, peak: 0)
    let speech = AudioSignalLevel(sampleCount: 960, rms: 0.01, peak: 0.05)

    try expectEqual(gate.shouldEmit(level: speech, frameLength: 960), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 2), false)
    try expectEqual(gate.shouldEmit(level: speech, frameLength: 960), true)
    try expectEqual(gate.trailingSilentFrames, 0)
  }

  static func configuresScreenCaptureKitSpeakerStream() throws {
    if #available(macOS 13.0, *) {
      let configuration = screenCaptureKitSpeakerStreamConfiguration()

      try expectEqual(configuration.width, 2)
      try expectEqual(configuration.height, 2)
      try expectEqual(configuration.capturesAudio, true)
      try expectEqual(configuration.excludesCurrentProcessAudio, true)
      try expectEqual(configuration.sampleRate, screenCaptureKitSpeakerSampleRate)
      try expectEqual(configuration.channelCount, Int(screenCaptureKitSpeakerChannelCount))
    }
  }

  static func configuresScreenCaptureKitSpeakerAudioFormat() throws {
    let format = try screenCaptureKitSpeakerAudioFormat()

    try expectEqual(Int(format.sampleRate), screenCaptureKitSpeakerSampleRate)
    try expectEqual(format.channelCount, screenCaptureKitSpeakerChannelCount)
    try expectEqual(format.commonFormat, .pcmFormatFloat32)
  }

  // MARK: - Transcript arbitration

  static func makeCandidate(
    language: String,
    text: String,
    isFinal: Bool = true,
    startMs: Int64 = 0,
    durationMs: Int64 = 2_000,
    confidence: Double? = nil,
    detectedLanguage: String? = nil,
    detectedLanguageConfidence: Double? = nil
  ) -> TranscriptCandidate {
    TranscriptCandidate(
      language: language,
      text: text,
      isFinal: isFinal,
      startMs: startMs,
      durationMs: durationMs,
      confidence: confidence,
      detectedLanguage: detectedLanguage,
      detectedLanguageConfidence: detectedLanguageConfidence
    )
  }

  static func overlapsCandidatesSharingMostOfTheirRange() throws {
    let english = makeCandidate(language: "en-US", text: "hello", startMs: 0, durationMs: 2_000)
    let japaneseOverlapping = makeCandidate(language: "ja-JP", text: "こんにちは", startMs: 400, durationMs: 1_800)
    let japaneseLater = makeCandidate(language: "ja-JP", text: "別の発話", startMs: 5_000, durationMs: 1_000)

    try expectEqual(candidateRangesOverlap(english, japaneseOverlapping), true)
    try expectEqual(candidateRangesOverlap(english, japaneseLater), false)
  }

  static func prefersConfidentCandidate() throws {
    let confident = makeCandidate(language: "en-US", text: "ship the panel", confidence: 0.85)
    let uncertain = makeCandidate(language: "ja-JP", text: "シップザパネル", confidence: 0.4)

    if compareTranscriptCandidates(confident, uncertain) <= 0 {
      throw TestFailure(message: "Expected the higher-confidence candidate to win")
    }
  }

  static func prefersScriptMatchingCandidate() throws {
    // Same speech recognized by both transcribers: the Japanese one produced
    // Japanese script, the English one produced Latin gibberish of Japanese.
    let japanese = makeCandidate(language: "ja-JP", text: "次のリリースは金曜日です")
    let english = makeCandidate(language: "en-US", text: "tsugi no ririsu wa kinyobi desu")

    if compareTranscriptCandidates(japanese, english) <= 0 {
      throw TestFailure(message: "Expected the script-matching candidate to win")
    }
  }

  static func penalizesImplausiblyDenseTranscript() throws {
    try expectEqual(
      transcriptDensityPenalty(
        makeCandidate(language: "ja-JP", text: String(repeating: "あ", count: 100), durationMs: 1_000)
      ) > 0,
      true
    )
    try expectEqual(
      transcriptDensityPenalty(
        makeCandidate(language: "ja-JP", text: "こんにちは", durationMs: 1_000)
      ),
      0
    )
  }

  static func combinesSequentialFinalsPerLanguage() throws {
    let first = makeCandidate(language: "ja-JP", text: "おはよう", startMs: 0, durationMs: 1_000)
    let second = makeCandidate(language: "ja-JP", text: "ございます", startMs: 1_100, durationMs: 900)

    guard let combined = combinedCandidate([second, first]) else {
      throw TestFailure(message: "Expected combined candidate")
    }

    try expectEqual(combined.text, "おはようございます")
    try expectEqual(combined.startMs, 0)
    try expectEqual(combined.durationMs, 2_000)

    let englishFirst = makeCandidate(language: "en-US", text: "good", startMs: 0, durationMs: 500)
    let englishSecond = makeCandidate(language: "en-US", text: "morning", startMs: 600, durationMs: 500)
    try expectEqual(combinedCandidate([englishFirst, englishSecond])?.text, "good morning")
  }

  static func arbitratesGroupAcrossLanguages() throws {
    let japanese = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは金曜日です",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.8
    )
    let english = makeCandidate(
      language: "en-US",
      text: "tsugi no ririsu",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.3
    )

    let winner = arbitrateGroup([english, japanese])
    try expectEqual(winner?.language, "ja-JP")
  }

  static func assignsWaveformBuckets() throws {
    try expectEqual(waveformBucketIndex(frame: 0, framesPerBucket: 100, bucketCount: 10), 0)
    try expectEqual(waveformBucketIndex(frame: 250, framesPerBucket: 100, bucketCount: 10), 2)
    try expectEqual(waveformBucketIndex(frame: 5_000, framesPerBucket: 100, bucketCount: 10), 9)
  }

  static func clampsMixedSamples() throws {
    try expectEqual(mixedSampleValue(0.5, 0.25), 0.75)
    try expectEqual(mixedSampleValue(0.9, 0.9), 1.0)
    try expectEqual(mixedSampleValue(-0.9, -0.9), -1.0)
  }

  actor OutputCollector {
    var outputs: [TranscriptArbiter.Output] = []

    func append(_ output: TranscriptArbiter.Output) {
      outputs.append(output)
    }

    func snapshot() -> [TranscriptArbiter.Output] {
      outputs
    }
  }

  static func arbiterSuppressesCrossLanguageInterimFlicker() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2) { output in
      await collector.append(output)
    }

    let english = makeCandidate(
      language: "en-US",
      text: "we should ship",
      isFinal: false,
      startMs: 0,
      durationMs: 1_500,
      confidence: 0.7
    )
    let japaneseMishearing = makeCandidate(
      language: "ja-JP",
      text: "ウィーシュッド",
      isFinal: false,
      startMs: 100,
      durationMs: 1_400,
      confidence: 0.3
    )

    await arbiter.receive(english)
    await arbiter.receive(japaneseMishearing)

    let outputs = await collector.snapshot()
    try expectEqual(outputs, [.interim(english)])
  }

  static func arbiterPairsFinalsAcrossLanguages() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 10_000) { output in
      await collector.append(output)
    }

    let japanese = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは金曜日です",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.8
    )
    let english = makeCandidate(
      language: "en-US",
      text: "tsugi no ririsu",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.3
    )

    await arbiter.receive(japanese)
    await arbiter.receive(english)

    let outputs = await collector.snapshot()
    try expectEqual(outputs.count, 1)
    guard case let .final(winner) = outputs[0] else {
      throw TestFailure(message: "Expected a final output")
    }
    try expectEqual(winner.language, "ja-JP")
  }

  static func arbiterFlushesUnpairedFinalAfterHold() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 50) { output in
      await collector.append(output)
    }

    let english = makeCandidate(
      language: "en-US",
      text: "only english heard this",
      startMs: 0,
      durationMs: 1_000,
      confidence: 0.9
    )

    await arbiter.receive(english)
    try expectEqual(await collector.snapshot(), [])

    try await Task.sleep(nanoseconds: 400_000_000)
    try expectEqual(await collector.snapshot(), [.final(english)])
  }

  static func arbiterDropsLateCounterpartFinalForFlushedUtterance() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 50) { output in
      await collector.append(output)
    }

    let english = makeCandidate(
      language: "en-US",
      text: "the release ships on friday",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.9
    )
    await arbiter.receive(english)
    try await Task.sleep(nanoseconds: 400_000_000)
    try expectEqual(await collector.snapshot(), [.final(english)])

    // The other language finalizes the same audio range after the hold
    // already flushed; it must not surface as a second bubble.
    let lateJapanese = makeCandidate(
      language: "ja-JP",
      text: "ザリリースシップスオンフライデー",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.8
    )
    await arbiter.receive(lateJapanese)
    try await Task.sleep(nanoseconds: 400_000_000)
    try expectEqual(await collector.snapshot(), [.final(english)])
  }

  static func arbiterExtendsHoldWhileCounterpartVolatileIsActive() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 80) { output in
      await collector.append(output)
    }

    // Japanese is still transcribing when the English mishearing finalizes;
    // the arbiter should wait for the Japanese final instead of flushing.
    let japaneseVolatile = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは",
      isFinal: false,
      startMs: 0,
      durationMs: 1_500,
      confidence: 0.5
    )
    let englishMishearing = makeCandidate(
      language: "en-US",
      text: "tsugi no ririsu",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.3
    )
    let japaneseFinal = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは金曜日です",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.8
    )

    await arbiter.receive(japaneseVolatile)
    await arbiter.receive(englishMishearing)
    try await Task.sleep(nanoseconds: 180_000_000)
    await arbiter.receive(japaneseFinal)
    try await Task.sleep(nanoseconds: 200_000_000)

    try expectEqual(
      await collector.snapshot(),
      [.interim(japaneseVolatile), .final(japaneseFinal)]
    )
  }
}

struct TestFailure: Error, CustomStringConvertible {
  let message: String

  var description: String {
    message
  }
}
