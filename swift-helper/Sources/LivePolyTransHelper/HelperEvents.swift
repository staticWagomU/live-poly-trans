import Foundation

/// Follow-up event carrying the translation of an already-emitted final
/// transcript. Emitted asynchronously so translation latency never blocks
/// the recognition result stream.
public struct TranslationEvent: Codable, Equatable, Sendable {
  public let type: String
  public let stream: String
  public let segmentId: String
  public let language: String
  public let targetLanguage: String
  public let trans: String
  public let timestamp: String
}

public func translationEvent(
  stream: AudioStream,
  segmentId: String,
  language: String,
  targetLanguage: String,
  translation: String,
  timestamp: Date = Date()
) -> TranslationEvent {
  TranslationEvent(
    type: "translation",
    stream: stream.rawValue,
    segmentId: segmentId,
    language: language,
    targetLanguage: targetLanguage,
    trans: translation,
    timestamp: iso8601Timestamp(timestamp)
  )
}

public struct AudioLevelEvent: Codable, Equatable, Sendable {
  public let type: String
  public let stream: String
  public let sampleCount: Int
  public let rms: Double
  public let peak: Double
  public let timestamp: String
}

public func audioLevelEvent(
  stream: AudioStream,
  level: AudioSignalLevel,
  timestamp: Date = Date()
) -> AudioLevelEvent {
  AudioLevelEvent(
    type: "audio-level",
    stream: stream.rawValue,
    sampleCount: level.sampleCount,
    rms: level.rms,
    peak: level.peak,
    timestamp: iso8601Timestamp(timestamp)
  )
}

/// Operational status the UI can surface (e.g. language pack download).
public struct StatusEvent: Codable, Equatable, Sendable {
  public let type: String
  public let stream: String
  public let state: String
  public let lang: String?

  public init(stream: AudioStream, state: String, lang: String? = nil) {
    self.type = "status"
    self.stream = stream.rawValue
    self.state = state
    self.lang = lang
  }
}

/// Serializes all stdout event emission. Transcript results, async
/// translation tasks, and status updates race otherwise and would interleave
/// partial JSON lines.
public actor HelperEventEmitter {
  private let segmentWriter: SegmentWriter?
  private var transcriptWriter: JsonlFileWriter?

  public init(segmentWriter: SegmentWriter?, transcriptWriter: JsonlFileWriter?) {
    self.segmentWriter = segmentWriter
    self.transcriptWriter = transcriptWriter
  }

  /// Recording start/stop swaps the per-recording transcript file while the
  /// stream keeps running; nil detaches it.
  public func setTranscriptWriter(_ writer: JsonlFileWriter?) {
    transcriptWriter = writer
  }

  public func emitInterim(_ event: TranscriptEvent) {
    emitLine(try? jsonLine(for: event))
  }

  public func emitFinal(_ event: TranscriptEvent, at timestamp: Date = Date()) {
    emitLine(try? jsonLine(for: event))
    try? segmentWriter?.write(event, at: timestamp)
    try? transcriptWriter?.writeLine(jsonLine(for: event))
  }

  public func emitTranslation(_ event: TranslationEvent) {
    emitLine(try? jsonLine(for: event))
    try? transcriptWriter?.writeLine(jsonLine(for: event))
  }

  public func emitAudioLevel(_ event: AudioLevelEvent) {
    emitLine(try? jsonLine(for: event))
  }

  public func emitStatus(_ event: StatusEvent) {
    emitLine(try? jsonLine(for: event))
  }

  private func emitLine(_ line: String?) {
    guard let line else {
      return
    }

    print(line)
    fflush(stdout)
  }
}

/// Append-only JSONL writer used for per-recording transcripts.
public final class JsonlFileWriter: @unchecked Sendable {
  private let url: URL
  private let lock = NSLock()

  public init?(path: String?) {
    guard let path, !path.isEmpty else {
      return nil
    }

    self.url = URL(fileURLWithPath: path)
  }

  public func writeLine(_ line: String) throws {
    lock.lock()
    defer { lock.unlock() }

    let data = Data("\(line)\n".utf8)
    if FileManager.default.fileExists(atPath: url.path) {
      let handle = try FileHandle(forWritingTo: url)
      try handle.seekToEnd()
      try handle.write(contentsOf: data)
      try handle.close()
    } else {
      try FileManager.default.createDirectory(
        at: url.deletingLastPathComponent(),
        withIntermediateDirectories: true
      )
      try data.write(to: url, options: .atomic)
    }
  }
}
