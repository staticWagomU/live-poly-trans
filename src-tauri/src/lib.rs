//! Tauri shell: UI commands feed the pipeline worker (pipeline.rs), which
//! owns the capture sessions (capture/), the ASR engines, the scheduler, and
//! the session recording (record.rs), and emits `transcript`/`status` events
//! back to the UI.

mod capture;
mod pipeline;
mod record;
mod translate;

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use kkm_core::language::LanguagePolicy;
use tauri::{Manager, State};

struct PipelineHandle {
    cmd_tx: Mutex<Sender<pipeline::Cmd>>,
    status: pipeline::StatusStore,
    policy: pipeline::PolicyStore,
}

impl PipelineHandle {
    fn send(&self, cmd: pipeline::Cmd) -> Result<(), String> {
        self.cmd_tx
            .lock()
            .unwrap()
            .send(cmd)
            .map_err(|_| "pipeline worker is gone".to_string())
    }
}

#[tauri::command]
fn start_capture(state: State<'_, PipelineHandle>) -> Result<(), String> {
    state.send(pipeline::Cmd::Start)
}

#[tauri::command]
fn stop_capture(state: State<'_, PipelineHandle>) -> Result<(), String> {
    state.send(pipeline::Cmd::Stop)
}

/// Snapshot for a webview that (re)loads mid-session: status events only
/// fire on change, so without this it would show "idle" until the next one.
#[tauri::command]
fn get_status(state: State<'_, PipelineHandle>) -> pipeline::StatusPayload {
    state.status.lock().unwrap().clone()
}

/// The language settings as the UI exchanges them. Kept here rather than
/// derived on [`LanguagePolicy`] so the core stays free of the shell's wire
/// format (ADR-153800).
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguagesPayload {
    /// Whisper language codes that may be spoken; one pins detection off.
    spoken: Vec<String>,
    /// Where translations go; `None` is "don't translate".
    target: Option<String>,
    /// Also translate the target language into the other spoken one.
    mutual: bool,
}

#[tauri::command]
fn get_languages(state: State<'_, PipelineHandle>) -> LanguagesPayload {
    let policy = state.policy.lock().unwrap();
    LanguagesPayload {
        spoken: policy.spoken.clone(),
        target: policy.target.clone(),
        mutual: policy.mutual,
    }
}

/// Takes effect from the next utterance — including mid-recording, which is
/// the point: a meeting that turns out to be in the other language should not
/// need to be restarted.
#[tauri::command]
fn set_languages(
    state: State<'_, PipelineHandle>,
    languages: LanguagesPayload,
) -> Result<(), String> {
    let mut spoken = languages.spoken;
    spoken.dedup();
    if spoken.is_empty() {
        return Err("at least one spoken language is required".into());
    }
    // Beyond two, detection on a one-second window is a guess rather than a
    // decision (docs/step0-results.md), so the setting simply does not exist.
    if spoken.len() > 2 {
        return Err("at most two spoken languages".into());
    }
    *state.policy.lock().unwrap() = LanguagePolicy {
        spoken,
        target: languages.target.filter(|t| !t.is_empty()),
        mutual: languages.mutual,
    };
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
            let status = pipeline::new_status_store();
            let policy = pipeline::new_policy_store();
            let ui = pipeline::Ui {
                app: app.handle().clone(),
                status: status.clone(),
            };
            let worker_policy = policy.clone();
            std::thread::spawn(move || pipeline::run(cmd_rx, ui, worker_policy));
            app.manage(PipelineHandle {
                cmd_tx: Mutex::new(cmd_tx),
                status,
                policy,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_capture,
            stop_capture,
            get_status,
            get_languages,
            set_languages
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
