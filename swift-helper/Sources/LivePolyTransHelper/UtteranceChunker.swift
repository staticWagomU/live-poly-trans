import Foundation

/// One silence-delimited stretch of speech, ready for whole-utterance
/// transcription. Timing is measured on the cumulative stream clock so
/// segment ids stay monotonic and aligned with the recording file.
public struct UtteranceChunk: Equatable, Sendable {
  public let samples: [Float]
  public let startMs: Int64
  public let durationMs: Int64

  public init(samples: [Float], startMs: Int64, durationMs: Int64) {
    self.samples = samples
    self.startMs = startMs
    self.durationMs = durationMs
  }
}

public func audioSignalRMS(_ samples: [Float]) -> Double {
  guard !samples.isEmpty else {
    return 0
  }

  var sumSquares = 0.0
  for sample in samples {
    let value = Double(sample)
    sumSquares += value * value
  }

  return (sumSquares / Double(samples.count)).squareRoot()
}

/// Cuts a continuous sample stream into utterances at trailing silence.
/// Unlike AudioSilenceGate this never drops audio from the clock: every
/// consumed sample advances the stream position, so chunk timestamps match
/// the pre-gate recording timeline.
public final class UtteranceChunker {
  private let sampleRate: Double
  private let silenceThresholdRMS: Double
  private let minSpeechFrames: Int
  private let trailingSilenceFrames: Int
  private let maxChunkFrames: Int
  private let preRollFrames: Int

  private var totalFrames: Int64 = 0
  private var preRoll: [Float] = []
  private var active: [Float] = []
  private var activeStartFrame: Int64 = 0
  private var speechFrames = 0
  private var trailingSilence = 0
  private var isActive = false

  public init(
    sampleRate: Double = 16_000,
    silenceThresholdRMS: Double,
    minSpeechMs: Int = 300,
    trailingSilenceMs: Int = 800,
    maxChunkMs: Int = 28_000,
    preRollMs: Int = 200
  ) {
    self.sampleRate = sampleRate
    self.silenceThresholdRMS = silenceThresholdRMS
    self.minSpeechFrames = frames(fromMs: minSpeechMs, sampleRate: sampleRate)
    self.trailingSilenceFrames = frames(fromMs: trailingSilenceMs, sampleRate: sampleRate)
    self.maxChunkFrames = frames(fromMs: maxChunkMs, sampleRate: sampleRate)
    self.preRollFrames = frames(fromMs: preRollMs, sampleRate: sampleRate)
  }

  public func consume(_ samples: [Float]) -> [UtteranceChunk] {
    guard !samples.isEmpty else {
      return []
    }

    let isSpeech = audioSignalRMS(samples) > silenceThresholdRMS
    let bufferStartFrame = totalFrames
    totalFrames += Int64(samples.count)

    if !isActive {
      guard isSpeech else {
        appendPreRoll(samples)
        return []
      }

      isActive = true
      activeStartFrame = bufferStartFrame - Int64(preRoll.count)
      active = preRoll
      preRoll = []
      speechFrames = 0
      trailingSilence = 0
    }

    active.append(contentsOf: samples)
    if isSpeech {
      speechFrames += samples.count
      trailingSilence = 0
    } else {
      trailingSilence += samples.count
    }

    if trailingSilence >= trailingSilenceFrames {
      return finishActive().map { [$0] } ?? []
    }

    if active.count >= maxChunkFrames {
      let chunk = finishActive()
      // The speaker is still mid-utterance; keep capturing seamlessly in a
      // fresh chunk instead of waiting for silence.
      isActive = true
      activeStartFrame = totalFrames
      return chunk.map { [$0] } ?? []
    }

    return []
  }

  /// Returns the pending utterance, if it contains enough speech; call on
  /// shutdown so the last utterance is not lost.
  public func flush() -> UtteranceChunk? {
    guard isActive else {
      return nil
    }

    return finishActive()
  }

  private func finishActive() -> UtteranceChunk? {
    defer {
      isActive = false
      active = []
      speechFrames = 0
      trailingSilence = 0
    }

    guard speechFrames >= minSpeechFrames else {
      return nil
    }

    return UtteranceChunk(
      samples: active,
      startMs: milliseconds(fromFrames: activeStartFrame),
      durationMs: milliseconds(fromFrames: Int64(active.count))
    )
  }

  private func appendPreRoll(_ samples: [Float]) {
    preRoll.append(contentsOf: samples)
    if preRoll.count > preRollFrames {
      preRoll.removeFirst(preRoll.count - preRollFrames)
    }
  }

  private func milliseconds(fromFrames frames: Int64) -> Int64 {
    Int64((Double(frames) * 1000 / sampleRate).rounded())
  }
}

private func frames(fromMs milliseconds: Int, sampleRate: Double) -> Int {
  Int((Double(milliseconds) * sampleRate / 1000).rounded())
}
