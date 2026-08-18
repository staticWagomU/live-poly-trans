//! The pipeline worker: one long-lived thread that owns the ASR engines,
//! the scheduler, and the capture-session lifecycle.
//!
//! Start/Stop arrive on a command channel and are handled strictly in
//! order, so a stale session can never overlap a new one, and the models
//! load exactly once for the app's lifetime instead of once per Record.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::capture;

/// Decode cadence, measured step-start to step-start: a slow decode eats
/// into the following idle time instead of stacking on top of it.
const STEP_INTERVAL: Duration = Duration::from_millis(1000);
/// How often the worker drains the ring buffer and checks for commands;
/// also the worst-case extra latency for Stop.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub enum Cmd {
    Start,
    Stop,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptPayload {
    committed_delta: String,
    volatile: String,
    utterance_final: Option<String>,
}

/// `state` drives the UI (idle/loading/listening/error); `message` is
/// human-readable detail, e.g. a non-fatal decode error while listening.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    state: &'static str,
    message: Option<String>,
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

fn emit_status(app: &AppHandle, state: &'static str, message: Option<String>) {
    let _ = app.emit("status", StatusPayload { state, message });
}

fn emit_step(app: &AppHandle, out: lpt_core::scheduler::StepOutput) {
    if out.committed_delta.is_empty() && out.volatile.is_empty() && out.utterance_final.is_none() {
        return; // nothing new — don't wake the UI every idle second
    }
    let _ = app.emit(
        "transcript",
        TranscriptPayload {
            committed_delta: out.committed_delta,
            volatile: out.volatile,
            utterance_final: out.utterance_final,
        },
    );
}

pub fn run(cmd_rx: Receiver<Cmd>, app: AppHandle) {
    let mut engines: Option<Engines> = None;
    emit_status(&app, "idle", None);
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            Cmd::Stop => {} // Stop while idle
            Cmd::Start => match run_session(&cmd_rx, &app, &mut engines) {
                Ok(()) => emit_status(&app, "idle", None),
                Err(e) => emit_status(&app, "error", Some(format!("{e:#}"))),
            },
        }
    }
}

fn load_engines<'a>(
    app: &AppHandle,
    engines: &'a mut Option<Engines>,
) -> anyhow::Result<&'a mut Engines> {
    if engines.is_none() {
        emit_status(app, "loading", None);
        let engine = lpt_whisper::WhisperEngine::load(&model_path(), &allowed_langs())?;
        let vad = lpt_whisper::SileroVad::load(&vad_model_path())?;
        *engines = Some(Engines { engine, vad });
    }
    Ok(engines.as_mut().expect("engines just ensured"))
}

fn run_session(
    cmd_rx: &Receiver<Cmd>,
    app: &AppHandle,
    engines: &mut Option<Engines>,
) -> anyhow::Result<()> {
    let engines = load_engines(app, engines)?;
    let stop_capture = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    capture::spawn(stop_capture.clone(), ready_tx);
    let result = (|| {
        let session = match ready_rx.recv() {
            Ok(session) => session?,
            Err(_) => anyhow::bail!("capture thread died before reporting"),
        };
        run_capture_loop(cmd_rx, app, engines, LaneRuntime::new(session)?)
    })();
    stop_capture.store(true, Ordering::SeqCst);
    result
}

/// Per-lane capture-to-scheduler state. Step 3 adds the speaker lane by
/// constructing a second runtime; the engines stay shared across lanes.
struct LaneRuntime {
    session: capture::CaptureSession,
    resampler: lpt_core::resample::StreamResampler,
    scheduler: lpt_core::scheduler::StreamScheduler,
    /// Downmix scratch buffer, reused across polls.
    mono: Vec<f32>,
    reported_drops: usize,
}

impl LaneRuntime {
    fn new(session: capture::CaptureSession) -> anyhow::Result<Self> {
        let resampler = lpt_core::resample::StreamResampler::new(session.src_rate)?;
        Ok(Self {
            session,
            resampler,
            scheduler: lpt_core::scheduler::StreamScheduler::new(),
            mono: Vec::new(),
            reported_drops: 0,
        })
    }

    /// Move captured audio into the scheduler and report overruns.
    fn pump(&mut self, app: &AppHandle) -> anyhow::Result<()> {
        self.drain();
        if !self.mono.is_empty() {
            let resampled = self.resampler.process(&self.mono)?;
            self.scheduler.push_audio(&resampled);
            self.mono.clear();
        }
        let drops = self.session.dropped.load(Ordering::Relaxed);
        if drops > self.reported_drops {
            emit_status(
                app,
                "listening",
                Some(format!("audio overrun: {drops} samples dropped")),
            );
            self.reported_drops = drops;
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

fn run_capture_loop(
    cmd_rx: &Receiver<Cmd>,
    app: &AppHandle,
    engines: &mut Engines,
    mut lane: LaneRuntime,
) -> anyhow::Result<()> {
    // Unset = auto-detect per utterance window (mixed ja/en meetings);
    // set LPT_LANG to pin a single language.
    let lang = std::env::var("LPT_LANG").ok();
    emit_status(app, "listening", None);
    let mut next_step = Instant::now() + STEP_INTERVAL;
    loop {
        match cmd_rx.recv_timeout(POLL_INTERVAL) {
            Ok(Cmd::Stop) => break,
            Ok(Cmd::Start) => {} // already running
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break, // app shutdown
        }
        lane.pump(app)?;
        if Instant::now() >= next_step {
            let started = Instant::now();
            match lane
                .scheduler
                .step(&mut engines.engine, &mut engines.vad, lang.as_deref())
            {
                Ok(Some(out)) => emit_step(app, out),
                Ok(None) => {}
                // Non-fatal: keep listening, surface the message.
                Err(e) => emit_status(app, "listening", Some(format!("decode error: {e:#}"))),
            }
            next_step = started + STEP_INTERVAL;
        }
    }
    if let Some(fin) = lane.scheduler.finish() {
        emit_step(app, fin);
    }
    Ok(())
}
