import Foundation

public let audioLevelEventMinimumInterval: TimeInterval = 0.1

public final class AudioLevelEventLimiter: @unchecked Sendable {
  private let minimumInterval: TimeInterval
  private let lock = NSLock()
  private var lastEmission: Date?

  public init(minimumInterval: TimeInterval) {
    self.minimumInterval = minimumInterval
  }

  public func shouldEmit(at timestamp: Date = Date()) -> Bool {
    lock.lock()
    defer { lock.unlock() }

    guard let lastEmission else {
      self.lastEmission = timestamp
      return true
    }

    guard timestamp.timeIntervalSince(lastEmission) >= minimumInterval else {
      return false
    }

    self.lastEmission = timestamp
    return true
  }
}
