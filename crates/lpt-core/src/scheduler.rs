//! Drives an [`AsrEngine`] over a rolling audio window and turns raw
//! hypotheses into committed/volatile text via [`LocalAgreement`].

use crate::local_agreement::LocalAgreement;
use crate::resample::TARGET_RATE;
use crate::AsrEngine;

/// Below this much audio a decode is wasted (whisper needs ~1s).
const MIN_WINDOW_SAMPLES: usize = TARGET_RATE as usize;

/// One decode step's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutput {
    pub committed_delta: String,
    pub volatile: String,
}

#[derive(Debug, Default)]
pub struct StreamScheduler {
    buffer: Vec<f32>,
    window_start: usize,
    agreement: LocalAgreement,
}

impl StreamScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_audio(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
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
        let hypothesis = engine.transcribe(window, lang)?;
        let agreement = self.agreement.feed(&hypothesis.text);
        Ok(Some(StepOutput {
            committed_delta: agreement.committed_delta,
            volatile: agreement.volatile,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hypothesis;

    /// Scripted engine: returns queued texts in order and records call sizes.
    struct FakeEngine {
        script: Vec<String>,
        received_samples: Vec<usize>,
    }

    impl FakeEngine {
        fn scripted(texts: &[&str]) -> Self {
            Self {
                script: texts.iter().rev().map(|s| s.to_string()).collect(),
                received_samples: Vec::new(),
            }
        }
    }

    impl AsrEngine for FakeEngine {
        fn transcribe(&mut self, samples: &[f32], _lang: Option<&str>) -> anyhow::Result<Hypothesis> {
            self.received_samples.push(samples.len());
            let text = self.script.pop().expect("script exhausted");
            Ok(Hypothesis {
                text,
                start_ms: 0,
                end_ms: (samples.len() * 1000 / TARGET_RATE as usize) as u64,
            })
        }
    }

    fn seconds(n: usize) -> Vec<f32> {
        vec![0.0; TARGET_RATE as usize * n]
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
    fn does_not_decode_below_one_second_of_audio() {
        let mut engine = FakeEngine::scripted(&[]);
        let mut sched = StreamScheduler::new();
        sched.push_audio(&seconds(1)[..100]);
        let out = sched.step(&mut engine, None).unwrap();
        assert_eq!(out, None);
        assert!(engine.received_samples.is_empty());
    }
}
