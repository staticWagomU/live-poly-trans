import Foundation

public struct WhisperEngineConfigInput: Encodable, Equatable, Sendable {
  public let type = "config"
  public let modelPath: String
  public let language: String
  public let sampleRate: Int
}

public struct WhisperEngineAudioInput: Encodable, Equatable, Sendable {
  public let type = "audio"
  public let stream: String
  public let seq: Int
  public let timestampMs: Int64
  public let pcm16Base64: String
}

public struct WhisperEngineFlushInput: Encodable, Equatable, Sendable {
  public let type = "flush"
  public let stream: String
}

public struct WhisperEngineShutdownInput: Encodable, Equatable, Sendable {
  public let type = "shutdown"
}

public struct WhisperEngineOutputLine: Codable, Equatable, Sendable {
  public let type: String
  public let state: String?
  public let message: String?
  public let fatal: Bool?
  public let stream: String?
  public let segmentId: String?
  public let startMs: Int64?
  public let durationMs: Int64?
  public let text: String?
  public let isFinal: Bool?
  public let language: String?
  public let confidence: Double?
}

public func whisperEngineConfigLine(
  modelPath: String,
  language: String,
  sampleRate: Int
) throws -> String {
  try jsonLine(for: WhisperEngineConfigInput(
    modelPath: modelPath,
    language: language,
    sampleRate: sampleRate
  ))
}

public func whisperEngineAudioLine(
  stream: AudioStream,
  seq: Int,
  timestampMs: Int64,
  samples: [Float]
) throws -> String {
  try jsonLine(for: WhisperEngineAudioInput(
    stream: stream.rawValue,
    seq: seq,
    timestampMs: timestampMs,
    pcm16Base64: pcm16Data(fromFloatSamples: samples).base64EncodedString()
  ))
}

public func whisperEngineFlushLine(stream: AudioStream) throws -> String {
  try jsonLine(for: WhisperEngineFlushInput(stream: stream.rawValue))
}

public func whisperEngineShutdownLine() throws -> String {
  try jsonLine(for: WhisperEngineShutdownInput())
}

public func pcm16Data(fromFloatSamples samples: [Float]) -> Data {
  var data = Data(capacity: samples.count * 2)
  for sample in samples {
    let clamped = min(1, max(-1, sample))
    var value = Int16((clamped * Float(Int16.max)).rounded()).littleEndian
    withUnsafeBytes(of: &value) { bytes in
      data.append(contentsOf: bytes)
    }
  }
  return data
}

public func transcriptEvent(
  from output: WhisperEngineOutputLine,
  stream: AudioStream,
  sourceLanguage: String?,
  targetLanguage: String?,
  timestamp: Date = Date()
) -> TranscriptEvent? {
  guard
    output.type == "transcript",
    let text = output.text?.trimmingCharacters(in: .whitespacesAndNewlines),
    isMeaningfulTranscript(text, isFinal: output.isFinal ?? false)
  else {
    return nil
  }

  let language = sessionLanguage(
    forWhisperLanguage: output.language ?? "auto",
    sourceLanguage: sourceLanguage,
    targetLanguage: targetLanguage,
    text: text
  )

  return transcriptEvent(
    stream: stream,
    language: language,
    text: text,
    translation: nil,
    isFinal: output.isFinal ?? false,
    timestamp: timestamp,
    segmentId: output.segmentId ?? "\(output.startMs ?? 0)-\(output.durationMs ?? 0)",
    confidence: output.confidence
  )
}

public func transcriptCandidate(
  from output: WhisperEngineOutputLine,
  sourceLanguage: String?,
  targetLanguage: String?
) -> TranscriptCandidate? {
  guard
    output.type == "transcript",
    let text = output.text?.trimmingCharacters(in: .whitespacesAndNewlines),
    let startMs = output.startMs,
    let durationMs = output.durationMs,
    isMeaningfulTranscript(text, isFinal: output.isFinal ?? false)
  else {
    return nil
  }

  return TranscriptCandidate(
    language: sessionLanguage(
      forWhisperLanguage: output.language ?? "auto",
      sourceLanguage: sourceLanguage,
      targetLanguage: targetLanguage,
      text: text
    ),
    text: text,
    isFinal: output.isFinal ?? false,
    startMs: startMs,
    durationMs: durationMs,
    confidence: output.confidence,
    detectedLanguage: output.language,
    detectedLanguageConfidence: nil
  )
}

public final class JsonLineAccumulator: @unchecked Sendable {
  private let lock = NSLock()
  private var pending = ""

  public init() {}

  public func consume(_ data: Data) -> [String] {
    lock.lock()
    defer { lock.unlock() }

    guard !data.isEmpty else {
      return finishLocked()
    }

    pending += String(decoding: data, as: UTF8.self)
    var lines: [String] = []
    while let newline = pending.firstIndex(of: "\n") {
      let line = String(pending[..<newline])
      pending.removeSubrange(...newline)
      if !line.isEmpty {
        lines.append(line)
      }
    }

    return lines
  }

  public func finish() -> [String] {
    lock.lock()
    defer { lock.unlock() }

    return finishLocked()
  }

  func finishLocked() -> [String] {
    guard !pending.isEmpty else {
      return []
    }

    let line = pending
    pending = ""
    return [line]
  }
}
