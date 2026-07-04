import Foundation
import Speech

@available(macOS 26.0, *)
public func installLanguageAsset(_ language: String) async throws {
  let supported = await SpeechTranscriber.supportedLocales
  guard let locale = supported.first(where: { $0.identifier(.bcp47) == language }) else {
    throw HelperRuntimeError.speechLanguageNotSupported(language)
  }

  let transcriber = SpeechTranscriber(
    locale: locale,
    transcriptionOptions: [],
    reportingOptions: [],
    attributeOptions: []
  )

  helperDebugLog("language-install-start language=\(language)")
  if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
    try await request.downloadAndInstall()
  }
  helperDebugLog("language-install-done language=\(language)")
}

/// Releases this app's asset reservation. macOS reclaims the model storage
/// on its own schedule, so the locale can linger in `installedLocales`.
@available(macOS 26.0, *)
public func uninstallLanguageAsset(_ language: String) async throws {
  let reserved = await AssetInventory.reservedLocales
  guard let locale = reserved.first(where: { $0.identifier(.bcp47) == language }) else {
    throw HelperRuntimeError.speechLanguageNotReserved(language)
  }

  helperDebugLog("language-uninstall-start language=\(language)")
  let released = await AssetInventory.release(reservedLocale: locale)
  helperDebugLog("language-uninstall-done language=\(language) released=\(released)")
}
