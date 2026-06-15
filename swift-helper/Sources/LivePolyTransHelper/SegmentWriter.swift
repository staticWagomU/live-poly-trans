import Foundation

public final class SegmentWriter: Sendable {
  private let directory: URL
  private let segmentDuration: TimeInterval
  private let lock = NSLock()

  public init?(directoryPath: String?, segmentDuration: TimeInterval = 15 * 60) {
    guard let directoryPath, !directoryPath.isEmpty else {
      return nil
    }

    self.directory = URL(fileURLWithPath: directoryPath, isDirectory: true)
    self.segmentDuration = segmentDuration
  }

  public func write(_ event: TranscriptEvent, at date: Date = Date()) throws {
    lock.lock()
    defer { lock.unlock() }

    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

    let stem = segmentStem(for: date)
    let jsonURL = directory.appendingPathComponent("\(stem).jsonl")
    let textURL = directory.appendingPathComponent("\(stem).txt")

    try append("\(try jsonLine(for: event))\n", to: jsonURL)

    let translation = event.trans.map { "\n  => \($0)" } ?? ""
    try append("[\(event.timestamp)] \(event.stream)/\(event.lang): \(event.text)\(translation)\n", to: textURL)
  }

  private func segmentStem(for date: Date) -> String {
    let bucket = floor(date.timeIntervalSince1970 / segmentDuration) * segmentDuration
    let bucketDate = Date(timeIntervalSince1970: bucket)
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime]
    return "segment-\(formatter.string(from: bucketDate).replacingOccurrences(of: ":", with: "-"))"
  }

  private func append(_ string: String, to url: URL) throws {
    let data = Data(string.utf8)
    if FileManager.default.fileExists(atPath: url.path) {
      let handle = try FileHandle(forWritingTo: url)
      try handle.seekToEnd()
      try handle.write(contentsOf: data)
      try handle.close()
    } else {
      try data.write(to: url, options: .atomic)
    }
  }
}
