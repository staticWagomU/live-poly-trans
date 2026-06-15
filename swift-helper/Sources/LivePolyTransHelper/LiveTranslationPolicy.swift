import Foundation

func textForFinalTranslation(_ text: String, isFinal: Bool) -> String? {
  guard isFinal else {
    return nil
  }

  let trimmedText = text.trimmingCharacters(in: .whitespacesAndNewlines)
  return trimmedText.isEmpty ? nil : trimmedText
}
