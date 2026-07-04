import Foundation
import Translation

@available(macOS 26.0, *)
public final class LiveTranslator: @unchecked Sendable {
  private let session: TranslationSession?

  public init(sourceLanguage: String?, targetLanguage: String?) {
    guard
      let sourceLanguage,
      let targetLanguage,
      sourceLanguage != targetLanguage
    else {
      self.session = nil
      return
    }

    self.session = TranslationSession(
      installedSource: Locale.Language(identifier: sourceLanguage),
      target: Locale.Language(identifier: targetLanguage)
    )
  }

  public func translate(_ text: String) async -> String? {
    guard let session, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
      return nil
    }

    do {
      return try await session.translate(text).targetText
    } catch {
      helperDebugLog("translation-skipped error=\(error)")
      return nil
    }
  }
}
