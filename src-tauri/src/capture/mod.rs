//! Audio capture sources. Each lane's backend owns its OS stream on its own
//! thread and hands interleaved f32 samples to the pipeline worker through a
//! lock-free ring buffer, so the pipeline sees one shape regardless of
//! whether the audio came from a microphone or the system mix.

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

pub mod mic;
#[cfg(target_os = "macos")]
pub mod speaker;

/// Seconds of interleaved audio the ring buffer can hold. The worker drains
/// every 100ms; this survives multi-second decode stalls without dropping.
pub const RING_CAPACITY_SECS: usize = 8;

pub struct CaptureSession {
    pub consumer: rtrb::Consumer<f32>,
    pub src_rate: u32,
    pub channels: usize,
    /// Samples the callback had to drop because the ring was full.
    pub dropped: Arc<AtomicUsize>,
    /// First stream error (device unplugged, format change, …). The stream
    /// keeps no audio flowing after one of these, so the pipeline must end
    /// the session instead of listening to silence forever.
    pub error: Arc<Mutex<Option<String>>>,
}
