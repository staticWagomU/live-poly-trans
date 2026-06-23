import Foundation

public func meetingSummaryPrompt(transcript: String) -> String {
  """
  You are helping summarize a live meeting transcript.
  Produce a concise Japanese meeting overview with:
  - one paragraph overview
  - decisions
  - action items
  - open questions

  Transcript:
  \(transcript)
  """
}

public func suggestedQuestionsPrompt(transcript: String) -> String {
  """
  会議の文字起こしを読んで、参加者が次に確認すると有用な質問を日本語で5件以内に提案してください。
  各質問は短く、会議内容に直接関係するものだけにしてください。

  Transcript:
  \(transcript)
  """
}

public func meetingQuestionPrompt(question: String, transcript: String) -> String {
  """
  Answer the user's question using only the meeting transcript.
  If the transcript does not contain enough evidence, say that the transcript does not confirm it.
  Answer in Japanese unless the question asks for another language.

  Question:
  \(question)

  Transcript:
  \(transcript)
  """
}
