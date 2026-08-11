//! LivePolyTrans core: engine-agnostic domain types and traits.
//!
//! UI shells and inference backends depend on this crate; it depends on
//! nothing platform-specific so it stays testable headless (ADR-153800).

pub mod local_agreement;
pub mod resample;
pub mod scheduler;

/// Which audio lane an event belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Mic,
    Speaker,
}

/// A transcription hypothesis for a window of audio.
#[derive(Debug, Clone, PartialEq)]
pub struct Hypothesis {
    pub text: String,
    /// Milliseconds from capture start.
    pub start_ms: u64,
    pub end_ms: u64,
    /// Language the engine detected (or was told) for this window.
    pub lang: Option<String>,
}

/// Streaming-agnostic ASR engine: transcribe one window of 16 kHz mono f32.
///
/// The LocalAgreement scheduler drives this repeatedly over a rolling
/// window; implementations must be safe to call from a dedicated thread.
pub trait AsrEngine: Send {
    fn transcribe(&mut self, samples: &[f32], lang: Option<&str>) -> anyhow::Result<Hypothesis>;
}

/// Sentence-level translator (queued, sequential; never blocks recognition).
pub trait Translator: Send {
    fn translate(&mut self, sentence: &str, source_lang: &str, target_lang: &str)
        -> anyhow::Result<String>;
}
