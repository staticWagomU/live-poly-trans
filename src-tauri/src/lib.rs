//! Tauri shell: wires mic capture (cpal) → resampler → StreamScheduler →
//! WhisperEngine, and emits `transcript`/`status` events to the UI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use tauri::{AppHandle, Emitter, State};

/// Decode cadence. Window decode itself costs ~0.7s (docs/step0-results.md),
/// so a shorter interval would only queue up work.
const STEP_INTERVAL: Duration = Duration::from_millis(1000);

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptPayload {
    committed_delta: String,
    volatile: String,
}

#[derive(Default)]
struct CaptureState {
    running: Arc<AtomicBool>,
}

fn model_path() -> String {
    std::env::var("LPT_WHISPER_MODEL").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../models/ggml-large-v3-turbo-q8_0.bin"
        )
        .to_string()
    })
}

#[tauri::command]
fn start_capture(app: AppHandle, state: State<'_, CaptureState>) -> Result<(), String> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let pending: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

    spawn_capture_thread(state.running.clone(), pending.clone(), app.clone());
    spawn_decode_thread(state.running.clone(), pending, app);
    Ok(())
}

#[tauri::command]
fn stop_capture(state: State<'_, CaptureState>) {
    state.running.store(false, Ordering::SeqCst);
}

fn spawn_capture_thread(
    running: Arc<AtomicBool>,
    pending: Arc<Mutex<Vec<f32>>>,
    app: AppHandle,
) {
    std::thread::spawn(move || {
        if let Err(e) = run_capture(&running, pending) {
            let _ = app.emit("status", format!("capture error: {e:#}"));
            running.store(false, Ordering::SeqCst);
        }
    });
}

/// cpal streams are !Send, so the stream lives entirely on this thread.
fn run_capture(running: &AtomicBool, pending: Arc<Mutex<Vec<f32>>>) -> anyhow::Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("no default input device")?;
    let config = device.default_input_config()?;
    anyhow::ensure!(
        config.sample_format() == cpal::SampleFormat::F32,
        "unsupported input sample format: {:?}",
        config.sample_format()
    );
    let src_rate = config.sample_rate();
    let channels = config.channels() as usize;

    let stream = device.build_input_stream(
        config.into(),
        move |data: &[f32], _info| {
            let mono: Vec<f32> = data
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect();
            let resampled = lpt_core::resample::resample_to_16k(&mono, src_rate);
            pending.lock().unwrap().extend_from_slice(&resampled);
        },
        |err| eprintln!("input stream error: {err}"),
        None,
    )?;
    stream.play()?;
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn spawn_decode_thread(running: Arc<AtomicBool>, pending: Arc<Mutex<Vec<f32>>>, app: AppHandle) {
    std::thread::spawn(move || {
        let _ = app.emit("status", "loading model…");
        let mut engine = match lpt_whisper::WhisperEngine::load(&model_path()) {
            Ok(engine) => engine,
            Err(e) => {
                let _ = app.emit("status", format!("model error: {e:#}"));
                running.store(false, Ordering::SeqCst);
                return;
            }
        };
        let _ = app.emit("status", "listening");
        // Unset = auto-detect per utterance window (mixed ja/en meetings);
        // set LPT_LANG to pin a single language.
        let lang = std::env::var("LPT_LANG").ok();
        let mut scheduler = lpt_core::scheduler::StreamScheduler::new();

        while running.load(Ordering::SeqCst) {
            std::thread::sleep(STEP_INTERVAL);
            {
                let mut queued = pending.lock().unwrap();
                scheduler.push_audio(&queued);
                queued.clear();
            }
            match scheduler.step(&mut engine, lang.as_deref()) {
                Ok(Some(out)) => {
                    let _ = app.emit(
                        "transcript",
                        TranscriptPayload {
                            committed_delta: out.committed_delta,
                            volatile: out.volatile,
                        },
                    );
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = app.emit("status", format!("decode error: {e:#}"));
                }
            }
        }
        let _ = app.emit("status", "idle");
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(CaptureState::default())
        .invoke_handler(tauri::generate_handler![start_capture, stop_capture])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
