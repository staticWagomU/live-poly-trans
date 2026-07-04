import AVFAudio
import CoreMedia
import Foundation
import Speech

@main
struct CommandLineOptionsTests {
  static func main() async throws {
    try parsesDetectLanguagesCommand()
    try parsesAiServerCommand()
    try parsesWaveformCommand()
    try parsesMixCommand()
    try parsesMicStreamCommandWithLocales()
    try parsesSpeakerStreamCommandWithSegmentDirectory()
    try parsesStreamCommandWithRecordingFiles()
    try readsValueAfterFlag()
    try readsRepeatedValuesAfterFlag()
    try formatsIso8601Timestamp()
    try formatsLanguageInfoWithBcp47Identifier()
    try encodesJsonLine()
    try formatsTranscriptEvent()
    try formatsTranslationEvent()
    try extractsTranscriptSpansFromSpeechAttributes()
    try detectsTranscriptLanguage()
    try buildsMeetingAiPrompts()
    try buildsRollingSummaryPrompt()
    try buildsChatPromptWithHistory()
    try decodesAiServerRequest()
    try await answersEmptyTranscriptWithoutModel()
    try requestsTranscriptConfidenceAttributes()
    try labelsSpeakerStreamAsSystemAudioSpeaker()
    try describesScreenCapturePermissionRecovery()
    try calculatesAudioFrameLength()
    try gatesLeadingAndTrailingSilence()
    try resumesAfterDroppedSilence()
    try configuresScreenCaptureKitSpeakerStream()
    try configuresScreenCaptureKitSpeakerAudioFormat()
    try overlapsCandidatesSharingMostOfTheirRange()
    try prefersConfidentCandidate()
    try prefersScriptMatchingCandidate()
    try penalizesImplausiblyDenseTranscript()
    try combinesSequentialFinalsPerLanguage()
    try arbitratesGroupAcrossLanguages()
    try assignsWaveformBuckets()
    try clampsMixedSamples()
    try await arbiterSuppressesCrossLanguageInterimFlicker()
    try await arbiterPairsFinalsAcrossLanguages()
    try await arbiterFlushesUnpairedFinalAfterHold()
    try await arbiterDropsLateCounterpartFinalForFlushedUtterance()
    try await arbiterExtendsHoldWhileCounterpartVolatileIsActive()
    print("all swift helper tests passed")
  }

  static func parsesDetectLanguagesCommand() throws {
    let options = try CommandLineOptions.parse(["helper", "--detect-languages"])
    try expectEqual(options.command, .detectLanguages)
  }

  static func parsesAiServerCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--ai-server"]).command,
      .aiServer
    )
  }

  static func parsesWaveformCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--waveform", "/tmp/mic.m4a"]).command,
      .waveform(path: "/tmp/mic.m4a", buckets: defaultWaveformBuckets)
    )
    try expectEqual(
      try CommandLineOptions.parse(["helper", "--waveform", "/tmp/mic.m4a", "--buckets", "300"]).command,
      .waveform(path: "/tmp/mic.m4a", buckets: 300)
    )
  }

  static func parsesMixCommand() throws {
    try expectEqual(
      try CommandLineOptions.parse([
        "helper",
        "--mix",
        "--input", "/tmp/mic.m4a",
        "--input", "/tmp/speaker.m4a",
        "--output", "/tmp/mixed.m4a"
      ]).command,
      .mix(inputs: ["/tmp/mic.m4a", "/tmp/speaker.m4a"], output: "/tmp/mixed.m4a")
    )

    do {
      _ = try CommandLineOptions.parse(["helper", "--mix", "--input", "/tmp/only-one.m4a"])
      throw TestFailure(message: "Expected mix parsing to fail without two inputs and an output")
    } catch is CommandLineOptionsError {
      // expected
    }
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

  static func parsesStreamCommandWithRecordingFiles() throws {
    let options = try CommandLineOptions.parse([
      "helper",
      "--stream",
      "mic",
      "--record-file",
      "/tmp/rec/mic.m4a",
      "--transcript-file",
      "/tmp/rec/mic.jsonl"
    ])

    try expectEqual(options.recordFile, "/tmp/rec/mic.m4a")
    try expectEqual(options.transcriptFile, "/tmp/rec/mic.jsonl")
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
    try expectEqual(event.speakerLabel, "Speaker A")
    try expectEqual(event.time, "1970-01-01T00:00:00Z")
    try expectEqual(event.timestamp, "1970-01-01T00:00:00Z")
    try expectEqual(event.confidence, 0.75)
    try expectEqual(event.detectedLang, "en")
    try expectEqual(event.spans, [TranscriptSpan(text: "hello", confidence: 0.75, startMs: 0, endMs: 500)])
  }

  static func formatsTranslationEvent() throws {
    let event = translationEvent(
      stream: .speaker,
      segmentId: "1200-800",
      language: "en-US",
      targetLanguage: "ja-JP",
      translation: "こんにちは",
      timestamp: Date(timeIntervalSince1970: 0)
    )

    try expectEqual(event.type, "translation")
    try expectEqual(event.stream, "speaker")
    try expectEqual(event.segmentId, "1200-800")
    try expectEqual(event.targetLanguage, "ja-JP")
    try expectEqual(event.trans, "こんにちは")
    try expectEqual(event.timestamp, "1970-01-01T00:00:00Z")
  }

  static func extractsTranscriptSpansFromSpeechAttributes() throws {
    guard #available(macOS 26.0, *) else {
      return
    }

    var text = AttributedString("hello")
    text[text.startIndex..<text.endIndex][AttributeScopes.SpeechAttributes.ConfidenceAttribute.self] = 0.75
    text[text.startIndex..<text.endIndex][AttributeScopes.SpeechAttributes.TimeRangeAttribute.self] = CMTimeRange(
      start: CMTime(value: 100, timescale: 1000),
      duration: CMTime(value: 500, timescale: 1000)
    )

    let spans = transcriptSpans(from: text)

    try expectEqual(spans, [TranscriptSpan(text: "hello", confidence: 0.75, startMs: 100, endMs: 600)])
    try expectEqual(transcriptConfidence(spans: spans), 0.75)
  }

  static func detectsTranscriptLanguage() throws {
    let detection = detectedTranscriptLanguage("今日はよろしくお願いします")
    try expectEqual(detection?.language, "ja")
  }

  static func buildsMeetingAiPrompts() throws {
    let transcript = "[2026-06-23T10:00:00Z] Speaker A / ja-JP: 次のリリースを決めます"

    let japaneseSummaryPrompt = meetingSummaryPrompt(
      transcript: transcript,
      previousSummary: nil,
      responseLanguage: "ja-JP"
    )

    if !japaneseSummaryPrompt.contains("Speaker A") {
      throw TestFailure(message: "Expected summary prompt to include transcript context")
    }

    if !japaneseSummaryPrompt.contains("Write the entire response in Japanese (ja-JP).") {
      throw TestFailure(message: "Expected summary prompt to require Japanese for Japanese main language")
    }

    if !japaneseSummaryPrompt.contains("Do not write the summary in English.") {
      throw TestFailure(message: "Expected summary prompt to forbid English when main language is Japanese")
    }

    if !japaneseSummaryPrompt.contains("概要") {
      throw TestFailure(message: "Expected Japanese section labels for Japanese main language")
    }

    let questionsPrompt = suggestedQuestionsPrompt(transcript: transcript, responseLanguage: "ja-JP")
    if !questionsPrompt.contains("Japanese (ja-JP)") {
      throw TestFailure(message: "Expected questions prompt to carry the response language")
    }
  }

  static func buildsRollingSummaryPrompt() throws {
    let prompt = meetingSummaryPrompt(
      transcript: "new lines",
      previousSummary: "- 概要: これまでの要約",
      responseLanguage: "ja-JP"
    )

    if !prompt.contains("Existing summary:") || !prompt.contains("これまでの要約") {
      throw TestFailure(message: "Expected rolling summary prompt to include the previous summary")
    }

    if !prompt.contains("New transcript portion:") {
      throw TestFailure(message: "Expected rolling summary prompt to label the new transcript portion")
    }
  }

  static func buildsChatPromptWithHistory() throws {
    let prompt = meetingQuestionPrompt(
      question: "リリース日は?",
      transcript: "transcript body",
      history: [AiChatTurn(question: "何を決めた?", answer: "リリース計画です")],
      responseLanguage: "ja-JP"
    )

    if !prompt.contains("リリース日は?") || !prompt.contains("transcript body") {
      throw TestFailure(message: "Expected answer prompt to include the question and transcript")
    }

    if !prompt.contains("Q: 何を決めた?") || !prompt.contains("A: リリース計画です") {
      throw TestFailure(message: "Expected answer prompt to include chat history")
    }

    if !prompt.contains("Japanese (ja-JP)") {
      throw TestFailure(message: "Expected answer prompt to carry the response language")
    }
  }

  static func decodesAiServerRequest() throws {
    let json = #"{"id":"7","command":"ask","question":"when?","language":"ja-JP","transcript":"body","history":[{"question":"q","answer":"a"}]}"#
    let request = try JSONDecoder().decode(AiServerRequest.self, from: Data(json.utf8))

    try expectEqual(request.id, "7")
    try expectEqual(request.command, "ask")
    try expectEqual(request.question, "when?")
    try expectEqual(request.history, [AiChatTurn(question: "q", answer: "a")])
  }

  static func answersEmptyTranscriptWithoutModel() async throws {
    let response = await aiServerResponse(
      for: AiServerRequest(id: "1", command: "summary", language: "ja-JP", transcript: "  ")
    )

    try expectEqual(response.ok, true)
    try expectEqual(response.response, "まだ確定済みの文字起こしがありません。")
  }

  static func requestsTranscriptConfidenceAttributes() throws {
    guard #available(macOS 26.0, *) else {
      return
    }

    let options = transcriptAttributeOptions()

    if !options.contains(.transcriptionConfidence) {
      throw TestFailure(message: "Expected transcript attributes to request confidence")
    }

    if !options.contains(.audioTimeRange) {
      throw TestFailure(message: "Expected transcript attributes to request audio time ranges")
    }
  }

  static func labelsSpeakerStreamAsSystemAudioSpeaker() throws {
    try expectEqual(speakerIdentifier(for: .speaker), "system-audio")
    try expectEqual(speakerLabel(for: .speaker), "Speaker B")
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

  // MARK: - Transcript arbitration

  static func makeCandidate(
    language: String,
    text: String,
    isFinal: Bool = true,
    startMs: Int64 = 0,
    durationMs: Int64 = 2_000,
    confidence: Double? = nil,
    detectedLanguage: String? = nil,
    detectedLanguageConfidence: Double? = nil
  ) -> TranscriptCandidate {
    TranscriptCandidate(
      language: language,
      text: text,
      isFinal: isFinal,
      startMs: startMs,
      durationMs: durationMs,
      confidence: confidence,
      detectedLanguage: detectedLanguage,
      detectedLanguageConfidence: detectedLanguageConfidence
    )
  }

  static func overlapsCandidatesSharingMostOfTheirRange() throws {
    let english = makeCandidate(language: "en-US", text: "hello", startMs: 0, durationMs: 2_000)
    let japaneseOverlapping = makeCandidate(language: "ja-JP", text: "こんにちは", startMs: 400, durationMs: 1_800)
    let japaneseLater = makeCandidate(language: "ja-JP", text: "別の発話", startMs: 5_000, durationMs: 1_000)

    try expectEqual(candidateRangesOverlap(english, japaneseOverlapping), true)
    try expectEqual(candidateRangesOverlap(english, japaneseLater), false)
  }

  static func prefersConfidentCandidate() throws {
    let confident = makeCandidate(language: "en-US", text: "ship the panel", confidence: 0.85)
    let uncertain = makeCandidate(language: "ja-JP", text: "シップザパネル", confidence: 0.4)

    if compareTranscriptCandidates(confident, uncertain) <= 0 {
      throw TestFailure(message: "Expected the higher-confidence candidate to win")
    }
  }

  static func prefersScriptMatchingCandidate() throws {
    // Same speech recognized by both transcribers: the Japanese one produced
    // Japanese script, the English one produced Latin gibberish of Japanese.
    let japanese = makeCandidate(language: "ja-JP", text: "次のリリースは金曜日です")
    let english = makeCandidate(language: "en-US", text: "tsugi no ririsu wa kinyobi desu")

    if compareTranscriptCandidates(japanese, english) <= 0 {
      throw TestFailure(message: "Expected the script-matching candidate to win")
    }
  }

  static func penalizesImplausiblyDenseTranscript() throws {
    try expectEqual(
      transcriptDensityPenalty(
        makeCandidate(language: "ja-JP", text: String(repeating: "あ", count: 100), durationMs: 1_000)
      ) > 0,
      true
    )
    try expectEqual(
      transcriptDensityPenalty(
        makeCandidate(language: "ja-JP", text: "こんにちは", durationMs: 1_000)
      ),
      0
    )
  }

  static func combinesSequentialFinalsPerLanguage() throws {
    let first = makeCandidate(language: "ja-JP", text: "おはよう", startMs: 0, durationMs: 1_000)
    let second = makeCandidate(language: "ja-JP", text: "ございます", startMs: 1_100, durationMs: 900)

    guard let combined = combinedCandidate([second, first]) else {
      throw TestFailure(message: "Expected combined candidate")
    }

    try expectEqual(combined.text, "おはようございます")
    try expectEqual(combined.startMs, 0)
    try expectEqual(combined.durationMs, 2_000)

    let englishFirst = makeCandidate(language: "en-US", text: "good", startMs: 0, durationMs: 500)
    let englishSecond = makeCandidate(language: "en-US", text: "morning", startMs: 600, durationMs: 500)
    try expectEqual(combinedCandidate([englishFirst, englishSecond])?.text, "good morning")
  }

  static func arbitratesGroupAcrossLanguages() throws {
    let japanese = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは金曜日です",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.8
    )
    let english = makeCandidate(
      language: "en-US",
      text: "tsugi no ririsu",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.3
    )

    let winner = arbitrateGroup([english, japanese])
    try expectEqual(winner?.language, "ja-JP")
  }

  static func assignsWaveformBuckets() throws {
    try expectEqual(waveformBucketIndex(frame: 0, framesPerBucket: 100, bucketCount: 10), 0)
    try expectEqual(waveformBucketIndex(frame: 250, framesPerBucket: 100, bucketCount: 10), 2)
    try expectEqual(waveformBucketIndex(frame: 5_000, framesPerBucket: 100, bucketCount: 10), 9)
  }

  static func clampsMixedSamples() throws {
    try expectEqual(mixedSampleValue(0.5, 0.25), 0.75)
    try expectEqual(mixedSampleValue(0.9, 0.9), 1.0)
    try expectEqual(mixedSampleValue(-0.9, -0.9), -1.0)
  }

  actor OutputCollector {
    var outputs: [TranscriptArbiter.Output] = []

    func append(_ output: TranscriptArbiter.Output) {
      outputs.append(output)
    }

    func snapshot() -> [TranscriptArbiter.Output] {
      outputs
    }
  }

  static func arbiterSuppressesCrossLanguageInterimFlicker() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2) { output in
      await collector.append(output)
    }

    let english = makeCandidate(
      language: "en-US",
      text: "we should ship",
      isFinal: false,
      startMs: 0,
      durationMs: 1_500,
      confidence: 0.7
    )
    let japaneseMishearing = makeCandidate(
      language: "ja-JP",
      text: "ウィーシュッド",
      isFinal: false,
      startMs: 100,
      durationMs: 1_400,
      confidence: 0.3
    )

    await arbiter.receive(english)
    await arbiter.receive(japaneseMishearing)

    let outputs = await collector.snapshot()
    try expectEqual(outputs, [.interim(english)])
  }

  static func arbiterPairsFinalsAcrossLanguages() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 10_000) { output in
      await collector.append(output)
    }

    let japanese = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは金曜日です",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.8
    )
    let english = makeCandidate(
      language: "en-US",
      text: "tsugi no ririsu",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.3
    )

    await arbiter.receive(japanese)
    await arbiter.receive(english)

    let outputs = await collector.snapshot()
    try expectEqual(outputs.count, 1)
    guard case let .final(winner) = outputs[0] else {
      throw TestFailure(message: "Expected a final output")
    }
    try expectEqual(winner.language, "ja-JP")
  }

  static func arbiterFlushesUnpairedFinalAfterHold() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 50) { output in
      await collector.append(output)
    }

    let english = makeCandidate(
      language: "en-US",
      text: "only english heard this",
      startMs: 0,
      durationMs: 1_000,
      confidence: 0.9
    )

    await arbiter.receive(english)
    try expectEqual(await collector.snapshot(), [])

    try await Task.sleep(nanoseconds: 400_000_000)
    try expectEqual(await collector.snapshot(), [.final(english)])
  }

  static func arbiterDropsLateCounterpartFinalForFlushedUtterance() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 50) { output in
      await collector.append(output)
    }

    let english = makeCandidate(
      language: "en-US",
      text: "the release ships on friday",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.9
    )
    await arbiter.receive(english)
    try await Task.sleep(nanoseconds: 400_000_000)
    try expectEqual(await collector.snapshot(), [.final(english)])

    // The other language finalizes the same audio range after the hold
    // already flushed; it must not surface as a second bubble.
    let lateJapanese = makeCandidate(
      language: "ja-JP",
      text: "ザリリースシップスオンフライデー",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.8
    )
    await arbiter.receive(lateJapanese)
    try await Task.sleep(nanoseconds: 400_000_000)
    try expectEqual(await collector.snapshot(), [.final(english)])
  }

  static func arbiterExtendsHoldWhileCounterpartVolatileIsActive() async throws {
    let collector = OutputCollector()
    let arbiter = TranscriptArbiter(languageCount: 2, holdMilliseconds: 80) { output in
      await collector.append(output)
    }

    // Japanese is still transcribing when the English mishearing finalizes;
    // the arbiter should wait for the Japanese final instead of flushing.
    let japaneseVolatile = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは",
      isFinal: false,
      startMs: 0,
      durationMs: 1_500,
      confidence: 0.5
    )
    let englishMishearing = makeCandidate(
      language: "en-US",
      text: "tsugi no ririsu",
      startMs: 100,
      durationMs: 1_900,
      confidence: 0.3
    )
    let japaneseFinal = makeCandidate(
      language: "ja-JP",
      text: "次のリリースは金曜日です",
      startMs: 0,
      durationMs: 2_000,
      confidence: 0.8
    )

    await arbiter.receive(japaneseVolatile)
    await arbiter.receive(englishMishearing)
    try await Task.sleep(nanoseconds: 180_000_000)
    await arbiter.receive(japaneseFinal)
    try await Task.sleep(nanoseconds: 200_000_000)

    try expectEqual(
      await collector.snapshot(),
      [.interim(japaneseVolatile), .final(japaneseFinal)]
    )
  }
}

struct TestFailure: Error, CustomStringConvertible {
  let message: String

  var description: String {
    message
  }
}
