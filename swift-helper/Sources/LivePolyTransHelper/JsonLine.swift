import Foundation

public func jsonLine<T: Encodable>(for payload: T) throws -> String {
  let encoder = JSONEncoder()
  encoder.outputFormatting = [.sortedKeys]

  let data = try encoder.encode(payload)
  return String(decoding: data, as: UTF8.self)
}
