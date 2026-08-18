//! Drives an [`AsrEngine`] over a rolling audio window and turns raw
//! hypotheses into committed/volatile text via [`LocalAgreement`].

use crate::local_agreement::LocalAgreement;
use crate::resample::TARGET_RATE;
use crate::{AsrEngine, SpeechDetector};

/// Below this much audio a decode is wasted (whisper needs ~1s).
const MIN_WINDOW_SAMPLES: usize = TARGET_RATE as usize;
/// Beyond this the window slides: re-decode cost grows with window length
/// (measured in docs/step0-results.md) and old audio no longer changes.
const MAX_WINDOW_SAMPLES: usize = 15 * TARGET_RATE as usize;
/// This much non-speech (per VAD) at the window tail marks an utterance
/// boundary: the hypothesis is stable there, so the volatile tail can be
/// flushed to committed and the window can slide without losing text.
const TRAILING_NON_SPEECH_SAMPLES: usize = TARGET_RATE as usize;
/// Upper bound on audio carried across a max-window slide. The decode
/// normally reaches within a second or two of the window end; a hypothesis
/// claiming otherwise has garbage timestamps, and carrying it all would
/// keep the window pinned at max size forever.
const MAX_CARRY_SAMPLES: usize = 5 * TARGET_RATE as usize;
/// Tail kept whenever non-speech audio is discarded. A VAD with a minimum
/// speech duration (Silero: 250ms by default) cannot see an utterance that
/// only just began, so dropping the whole window would clip its onset.
const ONSET_GUARD_SAMPLES: usize = 3 * TARGET_RATE as usize / 10;

/// One decode step's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutput {
    pub committed_delta: String,
    pub volatile: String,
    /// Full text of the utterance that just ended; present only on the step
    /// that crossed an utterance boundary (trailing silence or max window).
    /// This is the unit the translation lane consumes.
    pub utterance_final: Option<String>,
}

#[derive(Debug, Default)]
pub struct StreamScheduler {
    /// The current window: audio since the last slide.
    buffer: Vec<f32>,
    agreement: LocalAgreement,
    /// Language detected for the current window (auto mode only). Pinning it
    /// keeps hypotheses stable within a window while letting each new window
    /// (usually a new speaker turn) re-detect.
    window_lang: Option<String>,
    /// Text committed so far for the utterance in the current window; emitted
    /// whole as `utterance_final` when the utterance ends.
    utterance_acc: String,
}

impl StreamScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_audio(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
    }

    /// Slide the window: discard `buffer[..from]`, keep the rest, and start
    /// a fresh agreement context.
    fn slide_keeping(&mut self, from: usize) {
        self.buffer.drain(..from);
        self.agreement.reset();
        self.window_lang = None;
        self.utterance_acc.clear();
    }

    /// Commit the agreement's pending tail and take the finished utterance.
    /// Returns (newly committed tail, whole utterance if any).
    fn finalize_utterance(&mut self) -> (String, Option<String>) {
        let tail = self.agreement.flush();
        self.utterance_acc.push_str(&tail);
        let full = std::mem::take(&mut self.utterance_acc);
        let full = full.trim();
        (tail, (!full.is_empty()).then(|| full.to_string()))
    }

    /// End the stream (capture stopped): commit whatever text is still
    /// pending and reset for a fresh session. Returns None if nothing was
    /// pending.
    pub fn finish(&mut self) -> Option<StepOutput> {
        let (tail, utterance_final) = self.finalize_utterance();
        let all = self.buffer.len();
        self.slide_keeping(all);
        utterance_final.as_ref()?;
        Some(StepOutput {
            committed_delta: tail,
            volatile: String::new(),
            utterance_final,
        })
    }

    pub fn step(
        &mut self,
        engine: &mut dyn AsrEngine,
        vad: &mut dyn SpeechDetector,
        lang: Option<&str>,
    ) -> anyhow::Result<Option<StepOutput>> {
        if self.buffer.len() < MIN_WINDOW_SAMPLES {
            return Ok(None);
        }
        let window = self.buffer.as_slice();
        let window_len = window.len();
        let speech = vad.speech_segments(window)?;
        let Some(&(_, last_speech_end)) = speech.last() else {
            // No speech at all: drop the audio so silence and non-speech
            // noise never accumulate (or reach whisper). The onset-guard
            // tail survives in case an utterance just began there.
            self.slide_keeping(window_len.saturating_sub(ONSET_GUARD_SAMPLES));
            return Ok(None);
        };

        let effective_lang = lang.or(self.window_lang.as_deref());
        let hypothesis = engine.transcribe(window, effective_lang)?;
        // Pin only from a hypothesis that produced text: a decode whose
        // segments were all no-speech still reports a (meaningless) language.
        if lang.is_none() && self.window_lang.is_none() && !hypothesis.text.is_empty() {
            self.window_lang = hypothesis.lang.clone();
        }
        let agreement = self.agreement.feed(&hypothesis.text);
        let mut committed_delta = agreement.committed_delta;
        let mut volatile = agreement.volatile;
        self.utterance_acc.push_str(&committed_delta);

        let at_utterance_boundary =
            window_len.saturating_sub(last_speech_end) >= TRAILING_NON_SPEECH_SAMPLES;
        let mut utterance_final = None;
        if at_utterance_boundary || window_len >= MAX_WINDOW_SAMPLES {
            // The hypothesis is as stable as it will get: commit its tail
            // and hand the whole utterance downstream.
            let (tail, full) = self.finalize_utterance();
            committed_delta.push_str(&tail);
            volatile.clear();
            utterance_final = full;
            if at_utterance_boundary {
                // Everything after the last speech is silence — drop it,
                // except the onset guard (the VAD is blind to an utterance
                // that began in the window's final instants).
                self.slide_keeping(window_len.saturating_sub(ONSET_GUARD_SAMPLES));
            } else {
                // Forced slide mid-speech: the decode may not have reached
                // the window end, and audio past `end_ms` has no text yet.
                let end_sample = (hypothesis.end_ms as usize) * (TARGET_RATE as usize) / 1000;
                let keep_from = end_sample
                    .max(window_len.saturating_sub(MAX_CARRY_SAMPLES))
                    .min(window_len);
                self.slide_keeping(keep_from);
            }
        }
        Ok(Some(StepOutput {
            committed_delta,
            volatile,
            utterance_final,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hypothesis;

    /// Scripted engine: returns queued (text, detected lang, end_ms) in order
    /// and records call sizes and the lang argument it was given. An `end_ms`
    /// of None means "decoded the whole window".
    struct FakeEngine {
        script: Vec<(String, Option<String>, Option<u64>)>,
        received_samples: Vec<usize>,
        received_langs: Vec<Option<String>>,
    }

    impl FakeEngine {
        fn scripted(texts: &[&str]) -> Self {
            Self::scripted_with_langs(&texts.iter().map(|t| (*t, None)).collect::<Vec<_>>())
        }

        fn scripted_with_langs(entries: &[(&str, Option<&str>)]) -> Self {
            Self::new(entries.iter().map(|(t, l)| (*t, *l, None)).collect())
        }

        fn scripted_with_end_ms(entries: &[(&str, Option<u64>)]) -> Self {
            Self::new(entries.iter().map(|(t, e)| (*t, None, *e)).collect())
        }

        fn new(entries: Vec<(&str, Option<&str>, Option<u64>)>) -> Self {
            Self {
                script: entries
                    .iter()
                    .rev()
                    .map(|(t, l, e)| (t.to_string(), l.map(str::to_string), *e))
                    .collect(),
                received_samples: Vec::new(),
                received_langs: Vec::new(),
            }
        }
    }

    impl AsrEngine for FakeEngine {
        fn transcribe(&mut self, samples: &[f32], lang: Option<&str>) -> anyhow::Result<Hypothesis> {
            self.received_samples.push(samples.len());
            self.received_langs.push(lang.map(str::to_string));
            let (text, detected, end_ms) = self.script.pop().expect("script exhausted");
            Ok(Hypothesis {
                text,
                start_ms: 0,
                end_ms: end_ms
                    .unwrap_or((samples.len() * 1000 / TARGET_RATE as usize) as u64),
                lang: detected,
            })
        }
    }

    /// Audible fake audio: constant amplitude well above AmplitudeVad's bar.
    fn seconds(n: usize) -> Vec<f32> {
        vec![0.1; TARGET_RATE as usize * n]
    }

    /// Test stand-in for a real VAD: |sample| >= 0.05 counts as speech.
    struct AmplitudeVad;

    impl crate::SpeechDetector for AmplitudeVad {
        fn speech_segments(&mut self, samples: &[f32]) -> anyhow::Result<Vec<(usize, usize)>> {
            let mut segments = Vec::new();
            let mut start = None;
            for (i, s) in samples.iter().enumerate() {
                if s.abs() >= 0.05 {
                    start.get_or_insert(i);
                } else if let Some(begin) = start.take() {
                    segments.push((begin, i));
                }
            }
            if let Some(begin) = start {
                segments.push((begin, samples.len()));
            }
            Ok(segments)
        }
    }

    #[test]
    fn no_speech_window_is_dropped_except_the_onset_guard_tail() {
        let mut engine = FakeEngine::scripted(&["次"]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&vec![0.0005; 5 * TARGET_RATE as usize]); // noise only
        assert_eq!(sched.step(&mut engine, &mut vad, None).unwrap(), None);
        // the noise was dropped (bar the onset-guard tail): it never
        // accumulates and the next decode is mostly the new speech
        sched.push_audio(&seconds(2));
        sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(
            engine.received_samples,
            vec![ONSET_GUARD_SAMPLES + 2 * 16_000]
        );
    }

    /// Amplitude VAD that, like Silero, cannot see speech shorter than a
    /// minimum duration (here 0.5s).
    struct MinDurationVad;

    impl crate::SpeechDetector for MinDurationVad {
        fn speech_segments(&mut self, samples: &[f32]) -> anyhow::Result<Vec<(usize, usize)>> {
            let mut all = AmplitudeVad.speech_segments(samples)?;
            all.retain(|(s, e)| e - s >= TARGET_RATE as usize / 2);
            Ok(all)
        }
    }

    #[test]
    fn dropping_a_no_speech_window_keeps_a_just_started_utterance() {
        let mut engine = FakeEngine::scripted(&["もし"]);
        let mut vad = MinDurationVad;
        let mut sched = StreamScheduler::new();
        // 2s of near-silence, then an utterance begins in the last 200ms —
        // still below the VAD's minimum, so the window reads as "no speech".
        let mut audio = vec![0.0005; 2 * TARGET_RATE as usize];
        audio.extend(vec![0.1; TARGET_RATE as usize / 5]);
        sched.push_audio(&audio);
        assert_eq!(sched.step(&mut engine, &mut vad, None).unwrap(), None);
        // once the utterance continues, its first samples are still there
        sched.push_audio(&seconds(1));
        sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(
            engine.received_samples,
            vec![ONSET_GUARD_SAMPLES + 16_000]
        );
    }

    #[test]
    fn auto_mode_pins_detected_language_for_the_window() {
        let mut engine = FakeEngine::scripted_with_langs(&[
            ("Hello", Some("en")),
            ("Hello world", Some("en")),
            ("Hello world", Some("en")),
            ("こんにちは", Some("ja")),
        ]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(2));
        sched.step(&mut engine, &mut vad, None).unwrap(); // first decode: detect
        sched.push_audio(&seconds(1));
        sched.step(&mut engine, &mut vad, None).unwrap(); // pinned to detected lang
        sched.push_audio(&vec![0.0; TARGET_RATE as usize]); // pause → slide
        sched.step(&mut engine, &mut vad, None).unwrap();
        sched.push_audio(&seconds(2)); // next speaker, new window
        sched.step(&mut engine, &mut vad, None).unwrap(); // detection restarts
        assert_eq!(
            engine.received_langs,
            vec![None, Some("en".into()), Some("en".into()), None]
        );
    }

    #[test]
    fn auto_mode_does_not_pin_language_from_an_empty_hypothesis() {
        // A decode whose segments were all filtered as no-speech still
        // reports a detected language; pinning it would force e.g. "en"
        // (detected from a cough) onto the whole window.
        let mut engine = FakeEngine::scripted_with_langs(&[
            ("", Some("en")),
            ("こんにちは", Some("ja")),
            ("こんにちは", Some("ja")),
        ]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(2));
        sched.step(&mut engine, &mut vad, None).unwrap();
        sched.push_audio(&seconds(1));
        sched.step(&mut engine, &mut vad, None).unwrap();
        sched.push_audio(&seconds(1));
        sched.step(&mut engine, &mut vad, None).unwrap();
        // detection stays open until a hypothesis with text pins it
        assert_eq!(
            engine.received_langs,
            vec![None, None, Some("ja".into())]
        );
    }

    #[test]
    fn trailing_silence_flushes_volatile_and_slides() {
        let mut engine = FakeEngine::scripted(&["こんにちは"]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        let mut audio = vec![0.1; 2 * TARGET_RATE as usize]; // speech
        audio.extend(vec![0.0; TARGET_RATE as usize]); // 1s pause
        sched.push_audio(&audio);
        let out = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(out.committed_delta, "こんにちは"); // flushed, not stuck volatile
        assert_eq!(out.volatile, "");
        // window slid past the utterance: the silent remainder is gated
        sched.push_audio(&vec![0.0; TARGET_RATE as usize]);
        assert_eq!(sched.step(&mut engine, &mut vad, None).unwrap(), None);
        assert_eq!(engine.received_samples.len(), 1);
    }

    #[test]
    fn utterance_boundary_reports_the_full_utterance_for_translation() {
        let mut engine =
            FakeEngine::scripted(&["こんにちは、せ", "こんにちは、世界", "こんにちは、世界"]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(2));
        let first = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(first.utterance_final, None);
        sched.push_audio(&seconds(1));
        let second = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(second.utterance_final, None);
        sched.push_audio(&vec![0.0; TARGET_RATE as usize]); // 1s pause → boundary
        let third = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(third.committed_delta, "世界");
        assert_eq!(third.volatile, "");
        // the whole utterance, in one piece: the translation lane's input
        assert_eq!(third.utterance_final.as_deref(), Some("こんにちは、世界"));
    }

    #[test]
    fn silent_window_is_not_decoded() {
        let mut engine = FakeEngine::scripted(&[]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&vec![0.0005; 2 * TARGET_RATE as usize]); // ambient noise level
        let out = sched.step(&mut engine, &mut vad, None).unwrap();
        assert_eq!(out, None);
        assert!(engine.received_samples.is_empty());
    }

    #[test]
    fn consecutive_steps_commit_agreed_prefix() {
        let mut engine = FakeEngine::scripted(&["こんにちは、せ", "こんにちは、世界"]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(2));
        let first = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(first.committed_delta, "");
        assert_eq!(first.volatile, "こんにちは、せ");
        sched.push_audio(&seconds(1));
        let second = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(second.committed_delta, "こんにちは、");
        assert_eq!(second.volatile, "世界");
        // each step decodes the whole current window
        assert_eq!(engine.received_samples, vec![2 * 16_000, 3 * 16_000]);
    }

    #[test]
    fn window_slides_and_agreement_resets_after_max_window() {
        let mut engine = FakeEngine::scripted(&["長い発話です", "次"]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(16));
        // exceeds 15s max → volatile is flushed to committed, then slides
        let flushed = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(flushed.committed_delta, "長い発話です");
        assert_eq!(flushed.volatile, "");
        sched.push_audio(&seconds(2));
        let out = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(out.committed_delta, ""); // fresh agreement context
        assert_eq!(out.volatile, "次");
        // the post-slide decode saw only audio pushed after the slide
        assert_eq!(engine.received_samples, vec![16 * 16_000, 2 * 16_000]);
    }

    #[test]
    fn max_window_slide_carries_audio_the_decode_did_not_reach() {
        // 16s of speech, but the hypothesis says the decode only reached
        // 14s: the last 2s were never transcribed and must survive the slide.
        let mut engine =
            FakeEngine::scripted_with_end_ms(&[("長い発話", Some(14_000)), ("続き", None)]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(16));
        let flushed = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(flushed.committed_delta, "長い発話");
        // the carried 2s alone is a decodable window: no audio was lost
        let out = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(out.volatile, "続き");
        assert_eq!(engine.received_samples, vec![16 * 16_000, 2 * 16_000]);
    }

    #[test]
    fn finish_flushes_pending_text_as_a_final_utterance() {
        let mut engine = FakeEngine::scripted(&["こんにちは"]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(2));
        let out = sched.step(&mut engine, &mut vad, None).unwrap().unwrap();
        assert_eq!(out.volatile, "こんにちは");
        // stopping capture must not lose the volatile tail
        let fin = sched.finish().unwrap();
        assert_eq!(fin.committed_delta, "こんにちは");
        assert_eq!(fin.volatile, "");
        assert_eq!(fin.utterance_final.as_deref(), Some("こんにちは"));
        // and the scheduler is reset for the next session
        assert_eq!(sched.finish(), None);
    }

    #[test]
    fn does_not_decode_below_one_second_of_audio() {
        let mut engine = FakeEngine::scripted(&[]);
        let mut vad = AmplitudeVad;
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(1)[..100]);
        let out = sched.step(&mut engine, &mut vad, None).unwrap();
        assert_eq!(out, None);
        assert!(engine.received_samples.is_empty());
    }
}
