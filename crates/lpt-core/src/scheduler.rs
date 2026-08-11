//! Drives an [`AsrEngine`] over a rolling audio window and turns raw
//! hypotheses into committed/volatile text via [`LocalAgreement`].

use crate::local_agreement::LocalAgreement;
use crate::resample::TARGET_RATE;
use crate::AsrEngine;

/// Below this much audio a decode is wasted (whisper needs ~1s).
const MIN_WINDOW_SAMPLES: usize = TARGET_RATE as usize;
/// Beyond this the window slides: re-decode cost grows with window length
/// (measured in docs/step0-results.md) and old audio no longer changes.
const MAX_WINDOW_SAMPLES: usize = 15 * TARGET_RATE as usize;
/// Whisper hallucinates fixed phrases on (near-)silence, so windows quieter
/// than roughly -50 dBFS RMS are not worth decoding.
const SILENCE_RMS: f32 = 0.003;
/// This much silence at the window tail marks an utterance boundary: the
/// hypothesis is stable there, so the volatile tail can be flushed to
/// committed and the window can slide without losing text.
const TRAILING_SILENCE_SAMPLES: usize = TARGET_RATE as usize;

/// One decode step's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutput {
    pub committed_delta: String,
    pub volatile: String,
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

#[derive(Debug, Default)]
pub struct StreamScheduler {
    buffer: Vec<f32>,
    window_start: usize,
    agreement: LocalAgreement,
    /// Language detected for the current window (auto mode only). Pinning it
    /// keeps hypotheses stable within a window while letting each new window
    /// (usually a new speaker turn) re-detect.
    window_lang: Option<String>,
}

impl StreamScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_audio(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
    }

    /// Advance past all buffered audio and start a fresh agreement context.
    fn slide(&mut self) {
        self.buffer.clear();
        self.window_start = 0;
        self.agreement.reset();
        self.window_lang = None;
    }

    pub fn step(
        &mut self,
        engine: &mut dyn AsrEngine,
        lang: Option<&str>,
    ) -> anyhow::Result<Option<StepOutput>> {
        if self.buffer.len() - self.window_start < MIN_WINDOW_SAMPLES {
            return Ok(None);
        }
        let window = &self.buffer[self.window_start..];
        if rms(window) < SILENCE_RMS {
            return Ok(None);
        }
        let effective_lang = lang.or(self.window_lang.as_deref());
        let hypothesis = engine.transcribe(window, effective_lang)?;
        if lang.is_none() && self.window_lang.is_none() {
            self.window_lang = hypothesis.lang.clone();
        }
        let agreement = self.agreement.feed(&hypothesis.text);
        let mut committed_delta = agreement.committed_delta;
        let mut volatile = agreement.volatile;

        let tail = &window[window.len().saturating_sub(TRAILING_SILENCE_SAMPLES)..];
        let at_utterance_boundary = rms(tail) < SILENCE_RMS;
        if at_utterance_boundary || window.len() >= MAX_WINDOW_SAMPLES {
            committed_delta.push_str(&volatile);
            volatile.clear();
            self.slide();
        }
        Ok(Some(StepOutput {
            committed_delta,
            volatile,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hypothesis;

    /// Scripted engine: returns queued (text, detected lang) in order and
    /// records call sizes and the lang argument it was given.
    struct FakeEngine {
        script: Vec<(String, Option<String>)>,
        received_samples: Vec<usize>,
        received_langs: Vec<Option<String>>,
    }

    impl FakeEngine {
        fn scripted(texts: &[&str]) -> Self {
            Self::scripted_with_langs(&texts.iter().map(|t| (*t, None)).collect::<Vec<_>>())
        }

        fn scripted_with_langs(entries: &[(&str, Option<&str>)]) -> Self {
            Self {
                script: entries
                    .iter()
                    .rev()
                    .map(|(t, l)| (t.to_string(), l.map(str::to_string)))
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
            let (text, detected) = self.script.pop().expect("script exhausted");
            Ok(Hypothesis {
                text,
                start_ms: 0,
                end_ms: (samples.len() * 1000 / TARGET_RATE as usize) as u64,
                lang: detected,
            })
        }
    }

    /// Audible fake audio: constant amplitude well above the silence gate.
    fn seconds(n: usize) -> Vec<f32> {
        vec![0.1; TARGET_RATE as usize * n]
    }

    #[test]
    fn auto_mode_pins_detected_language_for_the_window() {
        let mut engine = FakeEngine::scripted_with_langs(&[
            ("Hello", Some("en")),
            ("Hello world", Some("en")),
            ("Hello world", Some("en")),
            ("こんにちは", Some("ja")),
        ]);
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(2));
        sched.step(&mut engine, None).unwrap(); // first decode: detect
        sched.push_audio(&seconds(1));
        sched.step(&mut engine, None).unwrap(); // pinned to detected lang
        sched.push_audio(&vec![0.0; TARGET_RATE as usize]); // pause → slide
        sched.step(&mut engine, None).unwrap();
        sched.push_audio(&seconds(2)); // next speaker, new window
        sched.step(&mut engine, None).unwrap(); // detection restarts
        assert_eq!(
            engine.received_langs,
            vec![None, Some("en".into()), Some("en".into()), None]
        );
    }

    #[test]
    fn trailing_silence_flushes_volatile_and_slides() {
        let mut engine = FakeEngine::scripted(&["こんにちは"]);
        let mut sched = StreamScheduler::new();
        let mut audio = vec![0.1; 2 * TARGET_RATE as usize]; // speech
        audio.extend(vec![0.0; TARGET_RATE as usize]); // 1s pause
        sched.push_audio(&audio);
        let out = sched.step(&mut engine, None).unwrap().unwrap();
        assert_eq!(out.committed_delta, "こんにちは"); // flushed, not stuck volatile
        assert_eq!(out.volatile, "");
        // window slid past the utterance: the silent remainder is gated
        sched.push_audio(&vec![0.0; TARGET_RATE as usize]);
        assert_eq!(sched.step(&mut engine, None).unwrap(), None);
        assert_eq!(engine.received_samples.len(), 1);
    }

    #[test]
    fn silent_window_is_not_decoded() {
        let mut engine = FakeEngine::scripted(&[]);
        let mut sched = StreamScheduler::new();
        sched.push_audio(&vec![0.0005; 2 * TARGET_RATE as usize]); // ambient noise level
        let out = sched.step(&mut engine, None).unwrap();
        assert_eq!(out, None);
        assert!(engine.received_samples.is_empty());
    }

    #[test]
    fn consecutive_steps_commit_agreed_prefix() {
        let mut engine = FakeEngine::scripted(&["こんにちは、せ", "こんにちは、世界"]);
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(2));
        let first = sched.step(&mut engine, None).unwrap().unwrap();
        assert_eq!(first.committed_delta, "");
        assert_eq!(first.volatile, "こんにちは、せ");
        sched.push_audio(&seconds(1));
        let second = sched.step(&mut engine, None).unwrap().unwrap();
        assert_eq!(second.committed_delta, "こんにちは、");
        assert_eq!(second.volatile, "世界");
        // each step decodes the whole current window
        assert_eq!(engine.received_samples, vec![2 * 16_000, 3 * 16_000]);
    }

    #[test]
    fn window_slides_and_agreement_resets_after_max_window() {
        let mut engine = FakeEngine::scripted(&["長い発話です", "次"]);
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(16));
        // exceeds 15s max → volatile is flushed to committed, then slides
        let flushed = sched.step(&mut engine, None).unwrap().unwrap();
        assert_eq!(flushed.committed_delta, "長い発話です");
        assert_eq!(flushed.volatile, "");
        sched.push_audio(&seconds(2));
        let out = sched.step(&mut engine, None).unwrap().unwrap();
        assert_eq!(out.committed_delta, ""); // fresh agreement context
        assert_eq!(out.volatile, "次");
        // the post-slide decode saw only audio pushed after the slide
        assert_eq!(engine.received_samples, vec![16 * 16_000, 2 * 16_000]);
    }

    #[test]
    fn does_not_decode_below_one_second_of_audio() {
        let mut engine = FakeEngine::scripted(&[]);
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(1)[..100]);
        let out = sched.step(&mut engine, None).unwrap();
        assert_eq!(out, None);
        assert!(engine.received_samples.is_empty());
    }
}
