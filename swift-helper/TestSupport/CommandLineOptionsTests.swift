import AVFAudio
import Foundation

@main
struct CommandLineOptionsTests {
  static func main() throws {
    try parsesDetectLanguagesCommand()
    try parsesMicStreamCommandWithLocales()
    try parsesSpeakerStreamCommandWithSegmentDirectory()
    try readsValueAfterFlag()
    try readsRepeatedValuesAfterFlag()
    try formatsIso8601Timestamp()
    try formatsLanguageInfoWithBcp47Identifier()
    try encodesJsonLine()
    try formatsTranscriptEvent()
    try labelsSpeakerStreamAsSystemAudioSpeaker()
    try translatesOnlyFinalTranscriptText()
    try logsOnlyFinalTranscriptResults()
    try describesScreenCapturePermissionRecovery()
    try calculatesAudioFrameLength()
    try gatesLeadingAndTrailingSilence()
    try resumesAfterDroppedSilence()
    try configuresScreenCaptureKitSpeakerStream()
    try configuresScreenCaptureKitSpeakerAudioFormat()
  }

  static func parsesDetectLanguagesCommand() throws {
    let options = try CommandLineOptions.parse(["helper", "--detect-languages"])
    try expectEqual(options.command, .detectLanguages)
  }

  static func parsesMicStreamCommandWithLocales() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream",
      "mic",
      "--source-language",
      "en-US",
      "--target-language",
      "ja-JP"
    ])

    try expectEqual(options.command, .stream(.mic))
    try expectEqual(options.sourceLanguage, "en-US")
    try expectEqual(options.targetLanguage, "ja-JP")
  }

  static func parsesSpeakerStreamCommandWithSegmentDirectory() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream",
      "speaker",
      "--language",
      "en-US",
      "--language",
      "ja-JP",
      "--segment-directory",
      "/tmp/live-poly-trans"
    ])

    try expectEqual(options.command, .stream(.speaker))
    try expectEqual(options.languages, ["en-US", "ja-JP"])
    try expectEqual(options.segmentDirectory, "/tmp/live-poly-trans")
  }

  static func expectEqual<T: Equatable>(_ actual: T, _ expected: T) throws {
    if actual != expected {
      throw TestFailure(message: "Expected \(expected), got \(actual)")
    }
  }

  static func readsValueAfterFlag() throws {
    try expectEqual(value(after: "--stream", in: ["helper", "--stream", "mic"]), "mic")
    try expectEqual(value(after: "--missing", in: ["helper", "--stream", "mic"]), nil)
  }

  static func readsRepeatedValuesAfterFlag() throws {
    try expectEqual(
      values(after: "--language", in: ["helper", "--language", "en-US", "--language", "ja-JP"]),
      ["en-US", "ja-JP"]
    )
  }

  static func formatsIso8601Timestamp() throws {
    try expectEqual(iso8601Timestamp(Date(timeIntervalSince1970: 0)), "1970-01-01T00:00:00Z")
  }

  static func formatsLanguageInfoWithBcp47Identifier() throws {
    let language = languageInfo(from: Locale(identifier: "ja_JP"))
    try expectEqual(language.id, "ja-JP")
    try expectEqual(language.label, "Japanese (Japan)")
  }

  static func encodesJsonLine() throws {
    let line = try jsonLine(for: LanguageInfo(id: "en-US", label: "English"))
    try expectEqual(line, #"{"id":"en-US","label":"English"}"#)
  }

  static func formatsTranscriptEvent() throws {
    let event = transcriptEvent(
      stream: .mic,
      language: "en-US",
      text: "hello",
      translation: nil,
      isFinal: true,
      timestamp: Date(timeIntervalSince1970: 0),
      segmentId: "0-1000",
      confidence: 0.75,
      spans: [
        TranscriptSpan(text: "hello", confidence: 0.75, startMs: 0, endMs: 500)
      ]
    )

    try expectEqual(event.type, "transcript")
    try expectEqual(event.stream, "mic")
    try expectEqual(event.speakerId, "self")
    try expectEqual(event.speakerLabel, "Mic")
    try expectEqual(event.time, "1970-01-01T00:00:00Z")
    try expectEqual(event.timestamp, "1970-01-01T00:00:00Z")
    try expectEqual(event.confidence, 0.75)
    try expectEqual(event.spans, [TranscriptSpan(text: "hello", confidence: 0.75, startMs: 0, endMs: 500)])
  }

  static func labelsSpeakerStreamAsSystemAudioSpeaker() throws {
    try expectEqual(speakerIdentifier(for: .speaker), "system-audio")
    try expectEqual(speakerLabel(for: .speaker), "Speaker")
  }

  static func translatesOnlyFinalTranscriptText() throws {
    try expectEqual(textForFinalTranslation(" hello ", isFinal: true), "hello")
    try expectEqual(textForFinalTranslation("hello", isFinal: false), nil)
  }

  static func logsOnlyFinalTranscriptResults() throws {
    try expectEqual(shouldLogTranscriptResult(isFinal: true), true)
    try expectEqual(shouldLogTranscriptResult(isFinal: false), false)
  }

  static func describesScreenCapturePermissionRecovery() throws {
    if !screenCapturePermissionRecoveryMessage.contains("Screen & System Audio Recording") {
      throw TestFailure(message: "Expected screen capture recovery message to name the settings pane")
    }

    if !screenCapturePermissionRecoveryMessage.contains("LivePolyTransHelper") {
      throw TestFailure(message: "Expected screen capture recovery message to name the helper app")
    }
  }

  static func calculatesAudioFrameLength() throws {
    guard
      let format = AVAudioFormat(
        commonFormat: .pcmFormatFloat32,
        sampleRate: 48_000,
        channels: 1,
        interleaved: false
      ),
      let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 960)
    else {
      throw TestFailure(message: "Could not create test audio buffer")
    }

    buffer.frameLength = 960
    try expectEqual(audioFrameLength(UnsafePointer(buffer.audioBufferList), format: format), 960)
  }

  static func gatesLeadingAndTrailingSilence() throws {
    let gate = AudioSilenceGate(silenceThresholdRMS: 0.0001, maxTrailingSilentFrames: 2_000)
    let silence = AudioSignalLevel(sampleCount: 960, rms: 0, peak: 0)
    let speech = AudioSignalLevel(sampleCount: 960, rms: 0.01, peak: 0.05)

    try expectEqual(gate.shouldEmit(level: silence, frameLength: 960), false)
    try expectEqual(gate.shouldEmit(level: speech, frameLength: 960), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 1_000), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 1_000), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 1), false)
  }

  static func resumesAfterDroppedSilence() throws {
    let gate = AudioSilenceGate(silenceThresholdRMS: 0.0001, maxTrailingSilentFrames: 1)
    let silence = AudioSignalLevel(sampleCount: 960, rms: 0, peak: 0)
    let speech = AudioSignalLevel(sampleCount: 960, rms: 0.01, peak: 0.05)

    try expectEqual(gate.shouldEmit(level: speech, frameLength: 960), true)
    try expectEqual(gate.shouldEmit(level: silence, frameLength: 2), false)
    try expectEqual(gate.shouldEmit(level: speech, frameLength: 960), true)
    try expectEqual(gate.trailingSilentFrames, 0)
  }

  static func configuresScreenCaptureKitSpeakerStream() throws {
    if #available(macOS 13.0, *) {
      let configuration = screenCaptureKitSpeakerStreamConfiguration()

      try expectEqual(configuration.width, 2)
      try expectEqual(configuration.height, 2)
      try expectEqual(configuration.capturesAudio, true)
      try expectEqual(configuration.excludesCurrentProcessAudio, true)
      try expectEqual(configuration.sampleRate, screenCaptureKitSpeakerSampleRate)
      try expectEqual(configuration.channelCount, Int(screenCaptureKitSpeakerChannelCount))
    }
  }

  static func configuresScreenCaptureKitSpeakerAudioFormat() throws {
    let format = try screenCaptureKitSpeakerAudioFormat()

    try expectEqual(Int(format.sampleRate), screenCaptureKitSpeakerSampleRate)
    try expectEqual(format.channelCount, screenCaptureKitSpeakerChannelCount)
    try expectEqual(format.commonFormat, .pcmFormatFloat32)
  }
}

struct TestFailure: Error, CustomStringConvertible {
  let message: String

  var description: String {
    message
  }
}
