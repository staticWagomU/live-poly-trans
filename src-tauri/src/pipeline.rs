//! The pipeline worker: one long-lived thread that owns the ASR engines,
//! the scheduler, and the capture-session lifecycle.
//!
//! Start/Stop arrive on a command channel and are handled strictly in
//! order, so a stale session can never overlap a new one, and the models
//! load exactly once for the app's lifetime instead of once per Record.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::capture;
use lpt_core::Lane;

/// Decode cadence, measured step-start to step-start: a slow decode eats
/// into the following idle time instead of stacking on top of it.
const STEP_INTERVAL: Duration = Duration::from_millis(1000);
/// How often the worker drains the ring buffer and checks for commands;
/// also the worst-case extra latency for Stop.
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// How long an overrun notice stays up after the last drop; long enough to
/// read, gone once the trouble has passed.
const OVERRUN_NOTICE: Duration = Duration::from_secs(5);

pub enum Cmd {
    Start,
    Stop,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptPayload {
    /// Which audio lane the text belongs to ("mic" now; "speaker" in Step 3).
    lane: &'static str,
    committed_delta: String,
    volatile: String,
    utterance_final: Option<String>,
}

/// `state` drives the UI (idle/loading/listening/error); `message` is
/// human-readable detail, e.g. a non-fatal decode error while listening.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPayload {
    state: &'static str,
    message: Option<String>,
}

/// Latest status snapshot for the `get_status` command. Status events only
/// fire on change, so a webview that (re)loads mid-session would otherwise
/// show "idle" until the next transition.
pub type StatusStore = Arc<Mutex<StatusPayload>>;

pub fn new_status_store() -> StatusStore {
    Arc::new(Mutex::new(StatusPayload {
        state: "idle",
        message: None,
    }))
}

/// The worker's channel back to the frontend: events plus the shared
/// status snapshot, updated together.
pub struct Ui {
    pub app: AppHandle,
    pub status: StatusStore,
}

struct Engines {
    engine: lpt_whisper::WhisperEngine,
    vad: lpt_whisper::SileroVad,
}

/// Languages the restricted auto-detection may choose between (Main/Sub pair).
fn allowed_langs() -> Vec<String> {
    std::env::var("LPT_LANGS")
        .unwrap_or_else(|_| "ja,en".into())
        .split(',')
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

// TODO(bundling): CARGO_MANIFEST_DIR is baked in at build time and only
// valid on the build machine. Resolve models via the app data dir (and the
// Step 2 ModelManager) before distributing bundles.
fn model_path() -> String {
    std::env::var("LPT_WHISPER_MODEL").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../models/ggml-large-v3-turbo-q8_0.bin"
        )
        .to_string()
    })
}

fn vad_model_path() -> String {
    std::env::var("LPT_VAD_MODEL").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../models/ggml-silero-v5.1.2.bin"
        )
        .to_string()
    })
}

fn lane_name(lane: Lane) -> &'static str {
    match lane {
        Lane::Mic => "mic",
        Lane::Speaker => "speaker",
    }
}

fn emit_status(ui: &Ui, state: &'static str, message: Option<String>) {
    let payload = StatusPayload { state, message };
    *ui.status.lock().unwrap() = payload.clone();
    let _ = ui.app.emit("status", payload);
}

/// Unconditional emit: pass outputs through an [`EmitGate`] first.
fn emit_step(ui: &Ui, lane: Lane, out: lpt_core::scheduler::StepOutput) {
    let _ = ui.app.emit(
        "transcript",
        TranscriptPayload {
            lane: lane_name(lane),
            committed_delta: out.committed_delta,
            volatile: out.volatile,
            utterance_final: out.utterance_final,
        },
    );
}

pub fn run(cmd_rx: Receiver<Cmd>, ui: Ui) {
    let mut engines: Option<Engines> = None;
    emit_status(&ui, "idle", None);
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            Cmd::Stop => {} // Stop while idle
            Cmd::Start => match run_session(&cmd_rx, &ui, &mut engines) {
                Ok(()) => emit_status(&ui, "idle", None),
                Err(e) => emit_status(&ui, "error", Some(format!("{e:#}"))),
            },
        }
    }
}

fn load_engines<'a>(
    ui: &Ui,
    engines: &'a mut Option<Engines>,
) -> anyhow::Result<&'a mut Engines> {
    if engines.is_none() {
        emit_status(ui, "loading", None);
        let engine = lpt_whisper::WhisperEngine::load(&model_path(), &allowed_langs())?;
        let vad = lpt_whisper::SileroVad::load(&vad_model_path())?;
        *engines = Some(Engines { engine, vad });
    }
    Ok(engines.as_mut().expect("engines just ensured"))
}

fn run_session(
    cmd_rx: &Receiver<Cmd>,
    ui: &Ui,
    engines: &mut Option<Engines>,
) -> anyhow::Result<()> {
    let engines = load_engines(ui, engines)?;
    let stop_capture = Arc::new(AtomicBool::new(false));
    let result = (|| {
        let mic = start_capture(capture::mic::spawn, &stop_capture)?;
        let mut lanes = vec![LaneRuntime::new(Lane::Mic, mic)?];
        // The speaker lane is best-effort: it needs macOS 14.2+ and the
        // system-audio permission, and a meeting is still worth
        // transcribing from the mic alone when it is unavailable.
        let mut notice = None;
        match start_speaker(&stop_capture) {
            Ok(Some(speaker)) => lanes.push(LaneRuntime::new(Lane::Speaker, speaker)?),
            Ok(None) => {}
            Err(e) => notice = Some(format!("speaker lane unavailable: {e:#}")),
        }
        run_capture_loop(cmd_rx, ui, engines, &mut lanes, notice)
    })();
    stop_capture.store(true, Ordering::SeqCst);
    result
}

/// Spawn a capture backend and wait for it to report its device config.
fn start_capture(
    spawn: fn(Arc<AtomicBool>, std::sync::mpsc::Sender<anyhow::Result<capture::CaptureSession>>),
    stop: &Arc<AtomicBool>,
) -> anyhow::Result<capture::CaptureSession> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    spawn(stop.clone(), ready_tx);
    match ready_rx.recv() {
        Ok(session) => session,
        Err(_) => anyhow::bail!("capture thread died before reporting"),
    }
}

/// `Ok(None)` on platforms with no loopback backend yet (Windows gets
/// WASAPI in a later pass).
#[cfg(target_os = "macos")]
fn start_speaker(
    stop: &Arc<AtomicBool>,
) -> anyhow::Result<Option<capture::CaptureSession>> {
    start_capture(capture::speaker::spawn, stop).map(Some)
}

#[cfg(not(target_os = "macos"))]
fn start_speaker(_stop: &Arc<AtomicBool>) -> anyhow::Result<Option<capture::CaptureSession>> {
    Ok(None)
}

/// Per-lane capture-to-scheduler state. Step 3 adds the speaker lane by
/// constructing a second runtime; the engines stay shared across lanes.
struct LaneRuntime {
    lane: Lane,
    session: capture::CaptureSession,
    resampler: lpt_core::resample::StreamResampler,
    scheduler: lpt_core::scheduler::StreamScheduler,
    /// Downmix scratch buffer, reused across polls.
    mono: Vec<f32>,
    reported_drops: usize,
    /// While set, an overrun notice is on screen; cleared when it expires.
    overrun_until: Option<Instant>,
    /// Per-lane: what the UI last saw of *this* lane's volatile tail.
    gate: EmitGate,
    /// When this lane is next due to decode. Per-lane so one lane's slow
    /// decode delays only itself.
    next_step: Instant,
}

impl LaneRuntime {
    fn new(lane: Lane, session: capture::CaptureSession) -> anyhow::Result<Self> {
        let resampler = lpt_core::resample::StreamResampler::new(session.src_rate)?;
        Ok(Self {
            lane,
            session,
            resampler,
            scheduler: lpt_core::scheduler::StreamScheduler::new(),
            mono: Vec::new(),
            reported_drops: 0,
            overrun_until: None,
            gate: EmitGate::default(),
            next_step: Instant::now() + STEP_INTERVAL,
        })
    }

    /// One poll of this lane: drain captured audio, then decode if its
    /// cadence came due.
    fn tick(
        &mut self,
        ui: &Ui,
        engines: &mut Engines,
        lang: Option<&str>,
    ) -> anyhow::Result<()> {
        self.pump(ui)?;
        if Instant::now() < self.next_step {
            return Ok(());
        }
        // Measured step-start to step-start: a slow decode eats into the
        // following idle time instead of stacking on top of it.
        let started = Instant::now();
        match self
            .scheduler
            .step(&mut engines.engine, &mut engines.vad, lang)
        {
            Ok(Some(out)) => {
                if self.gate.should_emit(&out) {
                    emit_step(ui, self.lane, out);
                }
            }
            Ok(None) => {}
            // Non-fatal: keep listening, surface the message.
            Err(e) => emit_status(ui, "listening", Some(format!("decode error: {e:#}"))),
        }
        self.next_step = started + STEP_INTERVAL;
        Ok(())
    }

    /// Stop: recover the audio still in flight (ring buffer → resampler
    /// tail → an undecoded scheduler remainder) before flushing the text.
    fn finish(
        &mut self,
        ui: &Ui,
        engines: &mut Engines,
        lang: Option<&str>,
    ) -> anyhow::Result<()> {
        self.pump(ui)?;
        let tail = self.resampler.flush()?;
        self.scheduler.push_audio(&tail);
        if let Some(fin) = self
            .scheduler
            .finish(&mut engines.engine, &mut engines.vad, lang)?
        {
            if self.gate.should_emit(&fin) {
                emit_step(ui, self.lane, fin);
            }
        }
        // A tail can still be on screen when finish had nothing to flush (the
        // scheduler discarded that hypothesis with a no-speech window drop).
        if self.gate.volatile_on_screen {
            emit_step(ui, self.lane, lpt_core::scheduler::StepOutput::default());
        }
        Ok(())
    }

    /// Move captured audio into the scheduler and report overruns. Fails
    /// when the stream itself failed (device unplugged): without that the
    /// session would keep "listening" to silence forever.
    fn pump(&mut self, ui: &Ui) -> anyhow::Result<()> {
        if let Some(msg) = self.session.error.lock().unwrap().take() {
            anyhow::bail!("input stream failed: {msg}");
        }
        self.drain();
        if !self.mono.is_empty() {
            let resampled = self.resampler.process(&self.mono)?;
            self.scheduler.push_audio(&resampled);
            self.mono.clear();
        }
        let drops = self.session.dropped.load(Ordering::Relaxed);
        if drops > self.reported_drops {
            // interleaved sample count → wall-clock duration of lost audio
            let ms =
                drops as u64 * 1000 / (self.session.channels as u64 * self.session.src_rate as u64);
            emit_status(
                ui,
                "listening",
                Some(format!("audio overrun: ~{ms}ms dropped so far")),
            );
            self.reported_drops = drops;
            self.overrun_until = Some(Instant::now() + OVERRUN_NOTICE);
        } else if self.overrun_until.is_some_and(|until| Instant::now() >= until) {
            // The overrun stopped a while ago; take the notice down.
            emit_status(ui, "listening", None);
            self.overrun_until = None;
        }
        Ok(())
    }

    /// Move everything the ring currently holds into `mono`, downmixed.
    fn drain(&mut self) {
        let channels = self.session.channels;
        let n = (self.session.consumer.slots() / channels) * channels;
        if n == 0 {
            return;
        }
        let Ok(chunk) = self.session.consumer.read_chunk(n) else {
            return;
        };
        let (a, b) = chunk.as_slices();
        // frames may straddle the two slices, so iterate their concatenation
        let mut samples = a.iter().chain(b.iter());
        self.mono.reserve(n / channels);
        for _ in 0..n / channels {
            let mut frame = 0.0f32;
            for _ in 0..channels {
                frame += samples.next().expect("n is a multiple of channels");
            }
            self.mono.push(frame / channels as f32);
        }
        chunk.commit_all();
    }
}

/// Decides which step outputs reach the UI. Idle steps are dropped, but an
/// all-empty output must still go out while a volatile tail is on screen:
/// LocalAgreement returns an empty volatile to *hide* a contradicted tail,
/// and swallowing that event would leave ghost text in the UI.
#[derive(Default)]
struct EmitGate {
    volatile_on_screen: bool,
}

impl EmitGate {
    fn should_emit(&mut self, out: &lpt_core::scheduler::StepOutput) -> bool {
        let has_news = !out.committed_delta.is_empty()
            || !out.volatile.is_empty()
            || out.utterance_final.is_some();
        let clears_tail = self.volatile_on_screen && out.volatile.is_empty();
        if !(has_news || clears_tail) {
            return false;
        }
        self.volatile_on_screen = !out.volatile.is_empty();
        true
    }
}

fn run_capture_loop(
    cmd_rx: &Receiver<Cmd>,
    ui: &Ui,
    engines: &mut Engines,
    lanes: &mut [LaneRuntime],
    notice: Option<String>,
) -> anyhow::Result<()> {
    // Unset = auto-detect per utterance window (mixed ja/en meetings);
    // set LPT_LANG to pin a single language.
    let lang = std::env::var("LPT_LANG").ok();
    emit_status(ui, "listening", notice);
    loop {
        match cmd_rx.recv_timeout(POLL_INTERVAL) {
            Ok(Cmd::Stop) => break,
            Ok(Cmd::Start) => {} // already running
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break, // app shutdown
        }
        for lane in lanes.iter_mut() {
            lane.tick(ui, engines, lang.as_deref())?;
        }
    }
    for lane in lanes.iter_mut() {
        lane.finish(ui, engines, lang.as_deref())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lpt_core::scheduler::StepOutput;

    fn out(committed: &str, volatile: &str) -> StepOutput {
        StepOutput {
            committed_delta: committed.into(),
            volatile: volatile.into(),
            utterance_final: None,
        }
    }

    #[test]
    fn gate_drops_idle_steps_but_passes_a_volatile_clear() {
        let mut gate = EmitGate::default();
        assert!(!gate.should_emit(&out("", ""))); // nothing shown yet: idle
        assert!(gate.should_emit(&out("", "こんにち"))); // tail appears
        // agreement hid the contradicted tail: the UI must be told
        assert!(gate.should_emit(&out("", "")));
        assert!(!gate.should_emit(&out("", ""))); // already clear: idle again
    }

    #[test]
    fn gate_tracks_the_tail_across_commits() {
        let mut gate = EmitGate::default();
        assert!(gate.should_emit(&out("", "こんにち")));
        assert!(gate.should_emit(&out("こんにちは、", "せ")));
        // a boundary flush ends with an empty tail; nothing stays on screen
        assert!(gate.should_emit(&out("世界", "")));
        assert!(!gate.should_emit(&out("", "")));
    }

    #[test]
    fn gate_passes_an_utterance_final_even_without_text_deltas() {
        let mut gate = EmitGate::default();
        let fin = StepOutput {
            committed_delta: String::new(),
            volatile: String::new(),
            utterance_final: Some("こんにちは".into()),
        };
        assert!(gate.should_emit(&fin));
    }
}
