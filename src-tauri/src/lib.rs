//! Tauri shell: UI commands feed the pipeline worker (pipeline.rs), which
//! owns the capture session (capture.rs), the ASR engines, and the
//! scheduler, and emits `transcript`/`status` events back to the UI.

mod capture;
mod pipeline;

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use tauri::{Manager, State};

struct PipelineHandle {
    cmd_tx: Mutex<Sender<pipeline::Cmd>>,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
            let handle = app.handle().clone();
            std::thread::spawn(move || pipeline::run(cmd_rx, handle));
            app.manage(PipelineHandle {
                cmd_tx: Mutex::new(cmd_tx),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![start_capture, stop_capture])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
