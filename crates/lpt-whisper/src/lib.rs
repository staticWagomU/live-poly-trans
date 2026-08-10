//! [`AsrEngine`] implementation over whisper.cpp (via whisper-rs, Metal).
//!
//! FFI boundary: correctness is verified by running the app and the spike
//! measurements (docs/step0-results.md), not unit tests.
//!
//! Must NOT be linked into the same binary as llama-cpp-2 until the ggml
//! symbol collision is resolved (docs/step0-results.md).

use anyhow::{Context, Result};
use lpt_core::{AsrEngine, Hypothesis};

pub struct WhisperEngine {
    ctx: whisper_rs::WhisperContext,
}

impl WhisperEngine {
    pub fn load(model_path: &str) -> Result<Self> {
        let mut params = whisper_rs::WhisperContextParameters::default();
        params.use_gpu(true);
        let ctx = whisper_rs::WhisperContext::new_with_params(model_path, params)
            .with_context(|| format!("load whisper model {model_path}"))?;
        Ok(Self { ctx })
    }
}

impl AsrEngine for WhisperEngine {
    fn transcribe(&mut self, samples: &[f32], lang: Option<&str>) -> Result<Hypothesis> {
        let mut state = self.ctx.create_state()?;
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(lang);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        state.full(params, samples)?;

        let mut text = String::new();
        let mut start_ms = 0u64;
        let mut end_ms = 0u64;
        for i in 0..state.full_n_segments() {
            if let Some(seg) = state.get_segment(i) {
                if i == 0 {
                    // whisper timestamps are centiseconds
                    start_ms = (seg.start_timestamp().max(0) * 10) as u64;
                }
                end_ms = (seg.end_timestamp().max(0) * 10) as u64;
                text.push_str(&seg.to_str_lossy()?);
            }
        }
        Ok(Hypothesis {
            text: text.trim().to_string(),
            start_ms,
            end_ms,
        })
    }
}
