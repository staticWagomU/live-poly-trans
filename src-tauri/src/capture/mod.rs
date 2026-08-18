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

/// Room for one anchor per callback. Callbacks run every ~10ms and the worker
/// drains every 100ms, so this holds several seconds of them.
pub const ANCHOR_CAPACITY: usize = 1024;

/// When a callback's first frame was captured, and where that frame sits in
/// the audio this lane has actually delivered.
///
/// Both backends timestamp against the same clock — mach host time — which is
/// what lets the lanes be placed on one timeline: cpal's macOS backend derives
/// its `StreamInstant` from `mHostTime`, and the Process Tap is handed the
/// same field directly. A Windows lane would have to agree with whatever clock
/// cpal uses there instead.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    /// Frames this lane delivered before the ones this anchor describes.
    /// Frames dropped to a full ring are *not* counted: the audio never
    /// reached the ring, so the gap belongs in the timeline, not hidden.
    pub frame: u64,
    /// Capture time of the first of those frames, in nanoseconds.
    pub nanos: u64,
}

pub struct CaptureSession {
    pub consumer: rtrb::Consumer<f32>,
    /// When the audio in `consumer` was captured. Drained alongside it.
    pub times: rtrb::Consumer<Anchor>,
    pub src_rate: u32,
    pub channels: usize,
    /// Samples the callback had to drop because the ring was full.
    pub dropped: Arc<AtomicUsize>,
    /// First stream error (device unplugged, format change, …). The stream
    /// keeps no audio flowing after one of these, so the pipeline must end
    /// the session instead of listening to silence forever.
    pub error: Arc<Mutex<Option<String>>>,
}
