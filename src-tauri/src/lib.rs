use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDetectionPayload {
    pub installed: Vec<LanguageInfo>,
    pub supported: Vec<LanguageInfo>,
}

#[derive(Default)]
pub struct HelperSession {
    pub child: Mutex<Option<Child>>,
}

pub fn helper_binary_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "helper-aarch64-apple-darwin"
    } else {
        "helper-x86_64-apple-darwin"
    }
}

pub fn helper_binary_candidates(manifest_dir: &Path, current_exe: &Path) -> Vec<PathBuf> {
    let binary = helper_binary_name();
    let exe_dir = current_exe.parent().unwrap_or_else(|| Path::new("."));

    vec![
        manifest_dir.join("binaries").join(binary),
        exe_dir.join(binary),
        exe_dir.join("../Resources").join(binary),
        exe_dir.join("../Resources/binaries").join(binary),
    ]
}

pub fn resolve_helper_path() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;

    helper_binary_candidates(&manifest_dir, &current_exe)
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| format!("helper binary was not found: {}", helper_binary_name()))
}

pub fn read_json_lines(app: AppHandle, stdout: impl std::io::Read + Send + 'static) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            match serde_json::from_str::<Value>(&line) {
                Ok(value) => {
                    let _ = app.emit("transcript-event", value);
                }
                Err(error) => {
                    let _ = app.emit("helper-error", format!("invalid helper JSON: {error}"));
                }
            }
        }
    });
}

pub fn read_stderr(app: AppHandle, stderr: impl std::io::Read + Send + 'static) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = app.emit("helper-error", line);
        }
    });
}

pub mod commands {
    use super::*;

    #[tauri::command]
    pub fn detect_languages() -> Result<LanguageDetectionPayload, String> {
        let output = Command::new(resolve_helper_path()?)
            .arg("--detect-languages")
            .output()
            .map_err(|error| error.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }

        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn start_microphone_session(
        app: AppHandle,
        state: State<'_, HelperSession>,
        source_language: String,
        target_language: String,
    ) -> Result<(), String> {
        stop_helper_child(&state)?;

        let mut child = Command::new(resolve_helper_path()?)
            .args([
                "--stream",
                "mic",
                "--source-language",
                &source_language,
                "--target-language",
                &target_language,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;

        if let Some(stdout) = child.stdout.take() {
            read_json_lines(app.clone(), stdout);
        }

        if let Some(stderr) = child.stderr.take() {
            read_stderr(app, stderr);
        }

        *state.child.lock().map_err(|error| error.to_string())? = Some(child);
        Ok(())
    }

    #[tauri::command]
    pub fn stop_microphone_session(state: State<'_, HelperSession>) -> Result<(), String> {
        stop_helper_child(&state)
    }

    pub fn stop_helper_child(state: &State<'_, HelperSession>) -> Result<(), String> {
        if let Some(mut child) = state.child.lock().map_err(|error| error.to_string())?.take() {
            child.kill().map_err(|error| error.to_string())?;
            let _ = child.wait();
        }

        Ok(())
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(HelperSession::default())
        .invoke_handler(tauri::generate_handler![
            commands::detect_languages,
            commands::start_microphone_session,
            commands::stop_microphone_session
        ])
        .setup(|app| {
            let _ = app.get_webview_window("main");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running LivePolyTrans");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_candidates_include_development_binary_path() {
        let candidates = helper_binary_candidates(
            Path::new("/repo/src-tauri"),
            Path::new("/repo/src-tauri/target/debug/live-poly-trans"),
        );

        assert!(candidates.contains(&PathBuf::from(
            "/repo/src-tauri/binaries/helper-aarch64-apple-darwin"
        )) || candidates.contains(&PathBuf::from(
            "/repo/src-tauri/binaries/helper-x86_64-apple-darwin"
        )));
    }
}
