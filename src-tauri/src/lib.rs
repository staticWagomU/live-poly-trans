//! Tauri shell: UI commands feed the pipeline worker (pipeline.rs), which
//! owns the capture sessions (capture/), the ASR engines, the scheduler, and
//! the session recording (record.rs), and emits `transcript`/`status` events
//! back to the UI.

mod capture;
mod library;
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
    output_device_prompt: pipeline::OutputDevicePromptStore,
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

#[tauri::command]
fn get_output_device_prompt(
    state: State<'_, PipelineHandle>,
) -> Option<pipeline::OutputDevicePrompt> {
    state.output_device_prompt.lock().unwrap().clone()
}

#[tauri::command]
fn respond_output_device_change(
    state: State<'_, PipelineHandle>,
    prompt_id: u64,
    switch_device: bool,
) -> Result<(), String> {
    state.send(pipeline::Cmd::RespondOutputDevice {
        prompt_id,
        switch_device,
    })
}

/// The sessions already on disk, newest first. Read on every visit to the
/// library rather than cached: the folder is the user's to move things in and
/// out of, and a list that disagrees with Finder would be worse than a
/// directory scan.
#[tauri::command]
fn list_recordings(app: tauri::AppHandle) -> Result<Vec<library::Recording>, String> {
    let base = library::base(&app).map_err(|e| format!("{e:#}"))?;
    Ok(library::list(&base))
}

/// A past session's transcript, joined back together from its append log.
/// Takes the directory the list handed out rather than a name, so a recording
/// reached from anywhere resolves the same way.
#[tauri::command]
fn read_recording(dir: String) -> Result<library::Session, String> {
    library::read(std::path::Path::new(&dir)).map_err(|e| format!("{e:#}"))
}

/// Change only the title users see. The timestamped directory remains the
/// session's stable identity and keeps its calendar placement intact.
#[tauri::command]
fn set_recording_title(
    app: tauri::AppHandle,
    dir: String,
    title: String,
) -> Result<String, String> {
    let base = library::base(&app).map_err(|e| format!("{e:#}"))?;
    library::set_title(&base, std::path::Path::new(&dir), &title).map_err(|e| format!("{e:#}"))
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
            let output_device_prompt = pipeline::new_output_device_prompt_store();
            let ui = pipeline::Ui {
                app: app.handle().clone(),
                status: status.clone(),
                output_device_prompt: output_device_prompt.clone(),
            };
            let worker_policy = policy.clone();
            std::thread::spawn(move || pipeline::run(cmd_rx, ui, worker_policy));
            app.manage(PipelineHandle {
                cmd_tx: Mutex::new(cmd_tx),
                status,
                policy,
                output_device_prompt,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_capture,
            stop_capture,
            get_status,
            get_output_device_prompt,
            respond_output_device_change,
            get_languages,
            set_languages,
            list_recordings,
            read_recording,
            set_recording_title
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
