//! [`AsrEngine`] implementation over whisper.cpp (via whisper-rs, Metal).
//!
//! FFI boundary: correctness is verified by running the app and the spike
//! measurements (docs/step0-results.md), not unit tests.
//!
//! Must NOT be linked into the same binary as llama-cpp-2 until the ggml
//! symbol collision is resolved (docs/step0-results.md).

use anyhow::{Context, Result};
use lpt_core::{AsrEngine, Hypothesis, SpeechDetector};

/// Segments whisper itself considers likely non-speech (coughs, breaths,
/// keyboard noise) hallucinate text; drop them above this probability.
const NO_SPEECH_THRESHOLD: f32 = 0.6;

pub struct WhisperEngine {
    ctx: whisper_rs::WhisperContext,
    /// When non-empty, auto-detection picks only among these languages
    /// (e.g. the app's Main/Sub pair) instead of whisper's full set.
    allowed_lang_ids: Vec<i32>,
}

/// Silero VAD via whisper.cpp. Runs on CPU so it never competes with the
/// whisper decode for GPU time.
///
/// Tuning (all optional, whisper.cpp defaults otherwise):
/// LPT_VAD_THRESHOLD (0..1, default 0.5), LPT_VAD_MIN_SPEECH_MS,
/// LPT_VAD_MIN_SILENCE_MS, LPT_VAD_PAD_MS.
pub struct SileroVad {
    ctx: whisper_rs::WhisperVadContext,
}

impl SileroVad {
    pub fn load(model_path: &str) -> Result<Self> {
        let mut params = whisper_rs::WhisperVadContextParams::new();
        params.set_n_threads(2);
        params.set_use_gpu(false);
        let ctx = whisper_rs::WhisperVadContext::new(model_path, params)
            .with_context(|| format!("load VAD model {model_path}"))?;
        Ok(Self { ctx })
    }

    fn params() -> whisper_rs::WhisperVadParams {
        let mut params = whisper_rs::WhisperVadParams::new();
        if let Some(v) = env_parse::<f32>("LPT_VAD_THRESHOLD") {
            params.set_threshold(v);
        }
        if let Some(v) = env_parse::<i32>("LPT_VAD_MIN_SPEECH_MS") {
            params.set_min_speech_duration(v);
        }
        if let Some(v) = env_parse::<i32>("LPT_VAD_MIN_SILENCE_MS") {
            params.set_min_silence_duration(v);
        }
        if let Some(v) = env_parse::<i32>("LPT_VAD_PAD_MS") {
            params.set_speech_pad(v);
        }
        params
    }
}

fn env_parse<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok()?.parse().ok()
}

impl SpeechDetector for SileroVad {
    fn speech_segments(&mut self, samples: &[f32]) -> Result<Vec<(usize, usize)>> {
        let segments = self.ctx.segments_from_samples(Self::params(), samples)?;
        // timestamps are centiseconds → x160 samples at 16 kHz
        Ok(segments
            .map(|seg| {
                (
                    ((seg.start * 160.0) as usize).min(samples.len()),
                    ((seg.end * 160.0) as usize).min(samples.len()),
                )
            })
            .collect())
    }
}

/// Pick the allowed language with the highest detection probability.
fn best_allowed_lang(probs: &[f32], allowed_ids: &[i32]) -> Option<i32> {
    allowed_ids
        .iter()
        .filter_map(|&id| probs.get(id as usize).map(|p| (id, *p)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

impl WhisperEngine {
    pub fn load(model_path: &str, allowed_langs: &[String]) -> Result<Self> {
        let mut params = whisper_rs::WhisperContextParameters::default();
        params.use_gpu(true);
        let ctx = whisper_rs::WhisperContext::new_with_params(model_path, params)
            .with_context(|| format!("load whisper model {model_path}"))?;
        let allowed_lang_ids = allowed_langs
            .iter()
            .map(|l| {
                whisper_rs::get_lang_id(l).with_context(|| format!("unknown language: {l}"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            ctx,
            allowed_lang_ids,
        })
    }
}

impl AsrEngine for WhisperEngine {
    fn transcribe(&mut self, samples: &[f32], lang: Option<&str>) -> Result<Hypothesis> {
        let mut state = self.ctx.create_state()?;

        // Restricted detection: pick the most probable of the allowed
        // languages, so e.g. a ja/en meeting can never drift into zh/ko.
        // The scheduler pins the result per window, so only the first decode
        // of each window pays the detection cost.
        let detected: Option<String> = match lang {
            Some(_) => None,
            None if !self.allowed_lang_ids.is_empty() => {
                state.pcm_to_mel(samples, 4)?;
                let (_, probs) = state.lang_detect(0, 4)?;
                best_allowed_lang(&probs, &self.allowed_lang_ids)
                    .and_then(whisper_rs::get_lang_str)
                    .map(str::to_string)
            }
            None => None,
        };

        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        // None here = whisper's unrestricted auto-detect.
        params.set_language(detected.as_deref().or(lang));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_nst(true); // no *cough* / event annotations
        state.full(params, samples)?;

        let mut text = String::new();
        let mut start_ms = None;
        let mut end_ms = 0u64;
        for i in 0..state.full_n_segments() {
            if let Some(seg) = state.get_segment(i) {
                if seg.no_speech_probability() > NO_SPEECH_THRESHOLD {
                    continue;
                }
                // whisper timestamps are centiseconds
                start_ms.get_or_insert((seg.start_timestamp().max(0) * 10) as u64);
                end_ms = (seg.end_timestamp().max(0) * 10) as u64;
                text.push_str(&seg.to_str_lossy()?);
            }
        }
        let start_ms = start_ms.unwrap_or(0);
        let detected = whisper_rs::get_lang_str(state.full_lang_id_from_state());
        Ok(Hypothesis {
            text: text.trim().to_string(),
            start_ms,
            end_ms,
            lang: detected.map(str::to_string),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_probability_among_allowed() {
        let mut probs = vec![0.0; 10];
        probs[1] = 0.9; // highest overall, but not an allowed language
        probs[3] = 0.2;
        probs[7] = 0.5;
        assert_eq!(best_allowed_lang(&probs, &[3, 7]), Some(7));
    }

    #[test]
    fn empty_allowed_set_returns_none() {
        assert_eq!(best_allowed_lang(&[0.1, 0.9], &[]), None);
    }
}
