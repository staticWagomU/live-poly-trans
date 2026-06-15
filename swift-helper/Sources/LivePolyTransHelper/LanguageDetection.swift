import Foundation

public struct LanguageInfo: Codable, Equatable {
  public let id: String
  public let label: String
}

public struct LanguageDetectionPayload: Codable, Equatable {
  public let installed: [LanguageInfo]
  public let supported: [LanguageInfo]
}

public func languageInfo(from locale: Locale) -> LanguageInfo {
  let id = locale.identifier(.bcp47)
  let label = Locale(identifier: "en_US").localizedString(forIdentifier: id) ?? id

  return LanguageInfo(id: id, label: label)
}

public func languageDetectionPayload(
  installed: [Locale],
  supported: [Locale]
) -> LanguageDetectionPayload {
  LanguageDetectionPayload(
    installed: installed.map(languageInfo(from:)).sorted { $0.id < $1.id },
    supported: supported.map(languageInfo(from:)).sorted { $0.id < $1.id }
  )
}
