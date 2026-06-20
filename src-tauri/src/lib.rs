use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};

const HELPER_DEBUG_PREFIX: &str = "live-poly-trans-helper debug:";

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
    pub children: Mutex<HashMap<String, Child>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedTranscriptMessage {
    pub role: String,
    #[serde(default, rename = "speakerId", skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    #[serde(
        default,
        rename = "speakerLabel",
        skip_serializing_if = "Option::is_none"
    )]
    pub speaker_label: Option<String>,
    pub language: String,
    pub text: String,
    pub translation: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTranscriptResult {
    pub json_path: String,
    pub text_path: String,
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
    let helper_app_binary = Path::new("LivePolyTransHelper.app")
        .join("Contents")
        .join("MacOS")
        .join("live-poly-trans-helper");

    vec![
        manifest_dir.join("binaries").join(&helper_app_binary),
        manifest_dir.join("binaries").join(binary),
        exe_dir.join(&helper_app_binary),
        exe_dir.join(binary),
        exe_dir.join("helper"),
        exe_dir.join("../Resources").join(&helper_app_binary),
        exe_dir.join("../Resources").join(binary),
        exe_dir.join("../Resources").join("helper"),
        exe_dir
            .join("../Resources/binaries")
            .join(&helper_app_binary),
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

pub fn read_json_lines(
    app: AppHandle,
    stdout: impl std::io::Read + Send + 'static,
    session_id: String,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            match serde_json::from_str::<Value>(&line) {
                Ok(mut value) => {
                    attach_session_id(&mut value, &session_id);
                    eprintln!(
                        "live-poly-trans tauri: transcript-event session={} stream={} lang={} final={} segment={} text={}",
                        value.get("sessionId").and_then(Value::as_str).unwrap_or("-"),
                        value.get("stream").and_then(Value::as_str).unwrap_or("-"),
                        value.get("lang").and_then(Value::as_str).unwrap_or("-"),
                        value.get("isFinal").and_then(Value::as_bool).unwrap_or(false),
                        value.get("segmentId").and_then(Value::as_str).unwrap_or("-"),
                        value.get("text").and_then(Value::as_str).unwrap_or("")
                    );
                    let _ = app.emit("transcript-event", value);
                }
                Err(error) => {
                    let _ = app.emit("helper-error", format!("invalid helper JSON: {error}"));
                }
            }
        }
    });
}

pub fn attach_session_id(value: &mut Value, session_id: &str) {
    if let Value::Object(object) = value {
        object.insert(
            "sessionId".to_string(),
            Value::String(session_id.to_string()),
        );
    }
}

pub fn read_stderr(app: AppHandle, stderr: impl std::io::Read + Send + 'static) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            eprintln!("{line}");
            if !line.starts_with(HELPER_DEBUG_PREFIX) {
                let _ = app.emit("helper-error", line);
            }
        }
    });
}

pub mod commands {
    use super::*;

    #[tauri::command]
    pub fn detect_languages() -> Result<LanguageDetectionPayload, String> {
        let helper_path = resolve_helper_path()?;
        eprintln!(
            "live-poly-trans tauri: detect-languages helper={}",
            helper_path.display()
        );

        let output = Command::new(helper_path)
            .arg("--detect-languages")
            .output()
            .map_err(|error| error.to_string())?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }

        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn start_stream_session(
        app: AppHandle,
        state: State<'_, HelperSession>,
        stream: String,
        source_language: String,
        target_language: String,
        languages: Vec<String>,
        session_id: String,
    ) -> Result<(), String> {
        stop_helper_child(&state, &stream)?;

        let segment_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join("segments")
            .join(&stream);
        fs::create_dir_all(&segment_dir).map_err(|error| error.to_string())?;

        let mut helper_args = vec![
            "--stream".to_string(),
            stream.clone(),
            "--source-language".to_string(),
            source_language,
            "--target-language".to_string(),
            target_language,
            "--segment-directory".to_string(),
            segment_dir.to_string_lossy().into_owned(),
        ];

        for language in languages {
            helper_args.push("--language".to_string());
            helper_args.push(language);
        }

        let helper_path = resolve_helper_path()?;
        eprintln!(
            "live-poly-trans tauri: start-stream stream={} helper={} segment_dir={} args={:?}",
            stream,
            helper_path.display(),
            segment_dir.display(),
            helper_args
        );

        let mut child = Command::new(helper_path)
            .args(helper_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        eprintln!(
            "live-poly-trans tauri: helper-started stream={} pid={}",
            stream,
            child.id()
        );

        if let Some(stdout) = child.stdout.take() {
            read_json_lines(app.clone(), stdout, session_id);
        }

        if let Some(stderr) = child.stderr.take() {
            read_stderr(app, stderr);
        }

        state
            .children
            .lock()
            .map_err(|error| error.to_string())?
            .insert(stream, child);
        Ok(())
    }

    #[tauri::command]
    pub fn stop_stream_session(
        state: State<'_, HelperSession>,
        stream: String,
    ) -> Result<(), String> {
        stop_helper_child(&state, &stream)
    }

    #[tauri::command]
    pub fn stop_all_sessions(state: State<'_, HelperSession>) -> Result<(), String> {
        let streams = state
            .children
            .lock()
            .map_err(|error| error.to_string())?
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        for stream in streams {
            stop_helper_child(&state, &stream)?;
        }

        Ok(())
    }

    #[tauri::command]
    pub fn save_transcript(
        app: AppHandle,
        messages: Vec<SavedTranscriptMessage>,
    ) -> Result<SaveTranscriptResult, String> {
        let transcript_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join("exports");
        fs::create_dir_all(&transcript_dir).map_err(|error| error.to_string())?;

        let timestamp = chrono_like_timestamp();
        let json_path = transcript_dir.join(format!("transcript-{timestamp}.json"));
        let text_path = transcript_dir.join(format!("transcript-{timestamp}.txt"));

        let json = serde_json::to_string_pretty(&messages).map_err(|error| error.to_string())?;
        fs::write(&json_path, json).map_err(|error| error.to_string())?;

        let mut text_file = File::create(&text_path).map_err(|error| error.to_string())?;
        for message in &messages {
            writeln!(
                text_file,
                "[{}] {} / {}: {}",
                message.timestamp,
                message.speaker_label.as_deref().unwrap_or(&message.role),
                message.language,
                message.text
            )
            .map_err(|error| error.to_string())?;

            if let Some(translation) = &message.translation {
                writeln!(text_file, "  => {translation}").map_err(|error| error.to_string())?;
            }
        }

        Ok(SaveTranscriptResult {
            json_path: json_path.display().to_string(),
            text_path: text_path.display().to_string(),
        })
    }

    pub fn stop_helper_child(state: &State<'_, HelperSession>, stream: &str) -> Result<(), String> {
        if let Some(mut child) = state
            .children
            .lock()
            .map_err(|error| error.to_string())?
            .remove(stream)
        {
            eprintln!(
                "live-poly-trans tauri: stop-stream stream={} pid={}",
                stream,
                child.id()
            );
            child.kill().map_err(|error| error.to_string())?;
            let _ = child.wait();
        }

        Ok(())
    }
}

fn chrono_like_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    seconds.to_string()
}

pub fn run() {
    tauri::Builder::default()
        .manage(HelperSession::default())
        .invoke_handler(tauri::generate_handler![
            commands::detect_languages,
            commands::start_stream_session,
            commands::stop_stream_session,
            commands::stop_all_sessions,
            commands::save_transcript
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
            "/repo/src-tauri/binaries/LivePolyTransHelper.app/Contents/MacOS/live-poly-trans-helper"
        )));
        assert!(
            candidates.contains(&PathBuf::from(
                "/repo/src-tauri/binaries/helper-aarch64-apple-darwin"
            )) || candidates.contains(&PathBuf::from(
                "/repo/src-tauri/binaries/helper-x86_64-apple-darwin"
            ))
        );
    }

    #[test]
    fn helper_candidates_include_bundled_sidecar_path() {
        let candidates = helper_binary_candidates(
            Path::new("/repo/src-tauri"),
            Path::new("/App/LivePolyTrans.app/Contents/MacOS/live-poly-trans"),
        );

        assert!(candidates.contains(&PathBuf::from(
            "/App/LivePolyTrans.app/Contents/MacOS/../Resources/LivePolyTransHelper.app/Contents/MacOS/live-poly-trans-helper"
        )));
        assert!(candidates.contains(&PathBuf::from(
            "/App/LivePolyTrans.app/Contents/MacOS/helper"
        )));
    }

    #[test]
    fn attach_session_id_tags_transcript_event_payloads() {
        let mut value = serde_json::json!({
            "type": "transcript",
            "stream": "mic",
            "text": "hello"
        });

        attach_session_id(&mut value, "mic-123");

        assert_eq!(value["sessionId"], "mic-123");
    }

    #[test]
    fn saved_messages_accept_native_speaker_labels() {
        let message: SavedTranscriptMessage = serde_json::from_value(serde_json::json!({
            "role": "speaker",
            "speakerId": "system-audio",
            "speakerLabel": "Speaker",
            "language": "en-US",
            "text": "hello",
            "translation": null,
            "timestamp": "2026-06-20T00:00:00Z"
        }))
        .unwrap();

        assert_eq!(message.speaker_id.as_deref(), Some("system-audio"));
        assert_eq!(message.speaker_label.as_deref(), Some("Speaker"));
    }
}
