import Foundation

public final class ISO8601TimestampFormatter: @unchecked Sendable {
  public static let shared = ISO8601TimestampFormatter()

  private let formatter = ISO8601DateFormatter()
  private let lock = NSLock()

  private init() {}

  public func string(from date: Date) -> String {
    lock.lock()
    defer { lock.unlock() }
    return formatter.string(from: date)
  }
}

public func iso8601Timestamp(_ date: Date) -> String {
  ISO8601TimestampFormatter.shared.string(from: date)
}
