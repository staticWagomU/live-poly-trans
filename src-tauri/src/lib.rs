use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub mod capture_helper_protocol;
pub mod whisper_engine_audio;
pub mod whisper_engine_backend;
pub mod whisper_engine_protocol;
pub mod whisper_engine_scheduler;
pub mod whisper_engine_sidecar;
pub mod whisper_engine_stabilization;
pub mod whisper_engine_whisper_cpp;

const HELPER_DEBUG_PREFIX: &str = "live-poly-trans-helper debug:";
const AI_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
// Must exceed the Swift helper's 5s shutdown safety net so the helper can
// finalize the m4a (moov atom) before we SIGKILL it.
const HELPER_STOP_GRACE: Duration = Duration::from_secs(7);
const WHISPER_STOP_GRACE: Duration = Duration::from_secs(15);
// How long start_recording_session waits for helpers to announce the stdin
// control channel. Helpers announce right after capture setup, so in practice
// this only delays a Record press that races the very first stream start.
const CONTROL_READY_TIMEOUT: Duration = Duration::from_secs(5);
pub const TRAY_WAVEFORM_ICON_SIZE: u32 = 18;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayWindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

fn read_overlay_window_state(path: &Path) -> Option<OverlayWindowState> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_overlay_window_state(path: &Path, state: &OverlayWindowState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())
}

fn overlay_window_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("overlay-window.json"))
}

fn capture_overlay_window_state(window: &WebviewWindow) -> Result<OverlayWindowState, String> {
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.inner_size().map_err(|error| error.to_string())?;
    Ok(OverlayWindowState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

fn apply_overlay_window_state(
    window: &WebviewWindow,
    state: &OverlayWindowState,
) -> Result<(), String> {
    window
        .set_position(Position::Physical(PhysicalPosition::new(state.x, state.y)))
        .map_err(|error| error.to_string())?;
    window
        .set_size(Size::Physical(PhysicalSize::new(state.width, state.height)))
        .map_err(|error| error.to_string())
}

#[derive(Default)]
pub struct OverlayAdjustmentState(Mutex<bool>);

pub const TRAY_COMMAND_TOGGLE_RECORDING: &str = "toggle-recording";
pub const TRAY_COMMAND_TOGGLE_PAUSE: &str = "toggle-pause";
pub const TRAY_COMMAND_TOGGLE_OVERLAY: &str = "toggle-overlay";
pub const TRAY_COMMAND_ADJUST_OVERLAY: &str = "adjust-overlay";

pub fn tray_command_for_menu_id(id: &str) -> Option<&'static str> {
    match id {
        "tray-toggle-recording" => Some(TRAY_COMMAND_TOGGLE_RECORDING),
        "tray-toggle-pause" => Some(TRAY_COMMAND_TOGGLE_PAUSE),
        "tray-toggle-overlay" => Some(TRAY_COMMAND_TOGGLE_OVERLAY),
        "tray-adjust-overlay" => Some(TRAY_COMMAND_ADJUST_OVERLAY),
        _ => None,
    }
}

pub fn shortcut_command(shortcut: &Shortcut) -> Option<&'static str> {
    let command_modifiers = Modifiers::ALT | Modifiers::SUPER;
    let control_modifiers = Modifiers::ALT | Modifiers::CONTROL;
    if shortcut.matches(command_modifiers, Code::KeyR)
        || shortcut.matches(control_modifiers, Code::KeyR)
    {
        return Some(TRAY_COMMAND_TOGGLE_RECORDING);
    }
    if shortcut.matches(command_modifiers, Code::KeyL)
        || shortcut.matches(control_modifiers, Code::KeyL)
    {
        return Some(TRAY_COMMAND_TOGGLE_OVERLAY);
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDetectionPayload {
    pub installed: Vec<LanguageInfo>,
    pub supported: Vec<LanguageInfo>,
    #[serde(default)]
    pub reserved: Vec<LanguageInfo>,
}

pub struct StreamChild {
    pub session_id: String,
    pub child: Child,
    /// How long to wait for a graceful exit before SIGKILL. Whisper sessions
    /// still run one final inference on the pending utterance after stdin
    /// closes, so they get a longer grace than the builtin engine.
    pub stop_grace: Duration,
    /// Set once the helper announces its stdin control channel; a helper
    /// binary from before the channel never sets it.
    pub control_ready: Arc<AtomicBool>,
}

pub fn stop_grace_for_engine(engine: Option<&str>) -> Duration {
    match engine {
        Some("whisper") => WHISPER_STOP_GRACE,
        _ => HELPER_STOP_GRACE,
    }
}

// MARK: whisper engine resolution

pub fn whisper_cli_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/bin/whisper-cli"),
        PathBuf::from("/usr/local/bin/whisper-cli"),
    ]
}

pub fn resolve_whisper_cli() -> Result<PathBuf, String> {
    whisper_cli_candidates()
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            "whisper-cli was not found. Install whisper.cpp first: brew install whisper-cpp"
                .to_string()
        })
}

pub fn whisper_engine_binary_name_for(
    target_arch: &str,
    target_os: &str,
    target_env: &str,
) -> String {
    match (target_arch, target_os, target_env) {
        ("aarch64", "macos", _) => "lpt-whisper-engine-aarch64-apple-darwin".to_string(),
        ("x86_64", "macos", _) => "lpt-whisper-engine-x86_64-apple-darwin".to_string(),
        ("x86_64", "windows", "msvc") => {
            "lpt-whisper-engine-x86_64-pc-windows-msvc.exe".to_string()
        }
        ("x86_64", "linux", _) => "lpt-whisper-engine-x86_64-unknown-linux-gnu".to_string(),
        _ => {
            let exe = if target_os == "windows" { ".exe" } else { "" };
            format!("lpt-whisper-engine-{target_arch}-{target_os}{exe}")
        }
    }
}

pub fn whisper_engine_binary_name() -> String {
    let target_env = if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else {
        ""
    };
    whisper_engine_binary_name_for(std::env::consts::ARCH, std::env::consts::OS, target_env)
}

pub fn whisper_engine_executable_name_for(target_os: &str) -> &'static str {
    if target_os == "windows" {
        "lpt-whisper-engine.exe"
    } else {
        "lpt-whisper-engine"
    }
}

pub fn whisper_engine_executable_name() -> &'static str {
    whisper_engine_executable_name_for(std::env::consts::OS)
}

pub fn whisper_engine_sidecar_candidates(manifest_dir: &Path, current_exe: &Path) -> Vec<PathBuf> {
    let binary = whisper_engine_binary_name();
    let executable = whisper_engine_executable_name();
    let exe_dir = current_exe.parent().unwrap_or_else(|| Path::new("."));

    vec![
        manifest_dir.join("binaries").join(&binary),
        manifest_dir.join("binaries").join(executable),
        manifest_dir.join("target/debug").join(executable),
        manifest_dir.join("target/release").join(executable),
        exe_dir.join(&binary),
        exe_dir.join(executable),
        exe_dir.join("../Resources").join(&binary),
        exe_dir.join("../Resources").join(executable),
        exe_dir.join("../Resources/binaries").join(&binary),
        exe_dir.join("../Resources/binaries").join(executable),
    ]
}

pub fn resolve_whisper_engine_sidecar() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;

    whisper_engine_sidecar_candidates(&manifest_dir, &current_exe)
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            format!(
                "whisper engine sidecar was not found: {}",
                whisper_engine_binary_name()
            )
        })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelInfo {
    pub file_name: String,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelsPayload {
    pub models: Vec<SpeechModelInfo>,
    pub cli_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedWhisperConfig {
    pub model_path: String,
    pub cli_path: String,
    pub engine_path: String,
}

pub fn superwhisper_models_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/superwhisper")
}

/// ggml-*.bin files in a directory; a missing directory (superwhisper not
/// installed) is just an empty list, not an error.
pub fn scan_speech_models(dir: &Path) -> Vec<SpeechModelInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut models = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !path.is_file()
            || !file_name.starts_with("ggml-")
            || path.extension().and_then(|ext| ext.to_str()) != Some("bin")
        {
            continue;
        }

        models.push(SpeechModelInfo {
            file_name: file_name.to_string(),
            path: path.display().to_string(),
            size_bytes: entry.metadata().map(|meta| meta.len()).unwrap_or(0),
        });
    }

    models.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    models
}

/// Validates a whisper session up front so a broken configuration fails the
/// invoke with a readable message instead of dying at the first utterance.
pub fn resolve_whisper_config(
    engine: Option<&str>,
    whisper_model: Option<&str>,
) -> Result<Option<ResolvedWhisperConfig>, String> {
    if engine != Some("whisper") {
        return Ok(None);
    }

    let model = whisper_model
        .filter(|model| !model.is_empty())
        .ok_or_else(|| "whisper engine requires a model path".to_string())?;
    if !Path::new(model).exists() {
        return Err(format!(
            "Whisper model was not found: {model}. It may have been moved or deleted; pick another model in Settings."
        ));
    }

    let cli = resolve_whisper_cli()?;
    let engine = resolve_whisper_engine_sidecar()?;
    Ok(Some(ResolvedWhisperConfig {
        model_path: model.to_string(),
        cli_path: cli.display().to_string(),
        engine_path: engine.display().to_string(),
    }))
}

#[derive(Default)]
pub struct HelperSession {
    pub children: Arc<Mutex<HashMap<String, StreamChild>>>,
    pub ai_server: Arc<Mutex<Option<AiServer>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedTranscriptSpan {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(default, rename = "startMs", skip_serializing_if = "Option::is_none")]
    pub start_ms: Option<i64>,
    #[serde(default, rename = "endMs", skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<i64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<SavedTranscriptSpan>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTranscriptResult {
    pub json_path: String,
    pub text_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChatTurn {
    pub question: String,
    pub answer: String,
}

// MARK: helper binary resolution

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

// MARK: stream helper process plumbing

pub fn read_json_lines(
    app: AppHandle,
    stdout: impl std::io::Read + Send + 'static,
    session_id: String,
    control_ready: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            match serde_json::from_str::<Value>(&line) {
                Ok(mut value) => {
                    if is_control_ready_event(&value) {
                        control_ready.store(true, Ordering::Relaxed);
                    }
                    attach_session_id(&mut value, &session_id);
                    if should_log_transcript_event(&value) {
                        eprintln!(
                            "live-poly-trans tauri: transcript-event session={} stream={} lang={} final=true segment={} chars={}",
                            value.get("sessionId").and_then(Value::as_str).unwrap_or("-"),
                            value.get("stream").and_then(Value::as_str).unwrap_or("-"),
                            value.get("lang").and_then(Value::as_str).unwrap_or("-"),
                            value.get("segmentId").and_then(Value::as_str).unwrap_or("-"),
                            value.get("text").and_then(Value::as_str).map(str::len).unwrap_or(0)
                        );
                    }
                    let _ = app.emit("transcript-event", value);
                }
                Err(error) => {
                    let _ = app.emit("helper-error", format!("invalid helper JSON: {error}"));
                }
            }
        }
    });
}

pub fn is_control_ready_event(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("status")
        && value.get("state").and_then(Value::as_str) == Some("control-ready")
}

pub fn should_log_transcript_event(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("transcript")
        && value
            .get("isFinal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
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

/// Polls the helper child so an unexpected crash surfaces as a
/// `helper-exited` event instead of the UI silently showing a dead session.
/// Intentional stops remove the map entry first, so no event is emitted.
fn watch_helper_exit(
    app: AppHandle,
    children: Arc<Mutex<HashMap<String, StreamChild>>>,
    stream: String,
    session_id: String,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));

        let exited = {
            let Ok(mut guard) = children.lock() else {
                return;
            };

            match guard.get_mut(&stream) {
                Some(entry) if entry.session_id == session_id => match entry.child.try_wait() {
                    Ok(Some(status)) => {
                        guard.remove(&stream);
                        Some(status.code())
                    }
                    Ok(None) => None,
                    Err(_) => return,
                },
                _ => return,
            }
        };

        if let Some(code) = exited {
            eprintln!(
                "live-poly-trans tauri: helper-exited stream={stream} session={session_id} code={code:?}"
            );
            let _ = app.emit(
                "helper-exited",
                serde_json::json!({
                    "stream": stream,
                    "sessionId": session_id,
                    "code": code,
                }),
            );
            return;
        }
    });
}

/// Closing stdin asks the helper to flush pending finals and finalize any
/// recording file; SIGKILL is the fallback, not the default.
pub fn stop_stream_child(
    children: &Mutex<HashMap<String, StreamChild>>,
    stream: &str,
) -> Result<(), String> {
    let entry = children
        .lock()
        .map_err(|error| error.to_string())?
        .remove(stream);

    let Some(mut entry) = entry else {
        return Ok(());
    };

    eprintln!(
        "live-poly-trans tauri: stop-stream stream={} pid={}",
        stream,
        entry.child.id()
    );
    drop(entry.child.stdin.take());

    let deadline = Instant::now() + entry.stop_grace;
    loop {
        match entry.child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error.to_string()),
        }
    }

    eprintln!("live-poly-trans tauri: stop-stream-force stream={stream}");
    let _ = entry.child.kill();
    let _ = entry.child.wait();
    Ok(())
}

pub fn stop_each_stream(
    streams: &[String],
    mut stop: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let errors = streams
        .iter()
        .filter_map(|stream| stop(stream).err().map(|error| format!("{stream}: {error}")))
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// Whisper engine paths, always resolved by this side so the helper carries
/// no defaults of its own.
pub struct WhisperEngineConfig<'a> {
    pub model_path: &'a str,
    pub cli_path: &'a str,
    pub engine_path: &'a str,
}

impl<'a> From<&'a ResolvedWhisperConfig> for WhisperEngineConfig<'a> {
    fn from(config: &'a ResolvedWhisperConfig) -> Self {
        Self {
            model_path: &config.model_path,
            cli_path: &config.cli_path,
            engine_path: &config.engine_path,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_stream_helper_args(
    stream: &str,
    source_language: &str,
    target_language: &str,
    languages: &[String],
    segment_dir: &str,
    record_file: Option<&str>,
    transcript_file: Option<&str>,
    whisper: Option<&WhisperEngineConfig>,
) -> Vec<String> {
    let mut args = vec![
        "--stream".to_string(),
        stream.to_string(),
        "--source-language".to_string(),
        source_language.to_string(),
        "--target-language".to_string(),
        target_language.to_string(),
        "--segment-directory".to_string(),
        segment_dir.to_string(),
    ];

    for language in languages {
        args.push("--language".to_string());
        args.push(language.clone());
    }

    if let Some(record_file) = record_file {
        args.push("--record-file".to_string());
        args.push(record_file.to_string());
    }

    if let Some(transcript_file) = transcript_file {
        args.push("--transcript-file".to_string());
        args.push(transcript_file.to_string());
    }

    if let Some(whisper) = whisper {
        args.push("--transcription-engine".to_string());
        args.push("whisper".to_string());
        args.push("--whisper-model".to_string());
        args.push(whisper.model_path.to_string());
        args.push("--whisper-cli".to_string());
        args.push(whisper.cli_path.to_string());
        args.push("--whisper-engine".to_string());
        args.push(whisper.engine_path.to_string());
    }

    args
}

// MARK: AI server (persistent helper process)

#[derive(Debug, Serialize)]
struct AiServerRequestPayload<'a> {
    id: String,
    command: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    question: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<&'a str>,
    #[serde(rename = "previousSummary", skip_serializing_if = "Option::is_none")]
    previous_summary: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    history: Option<&'a [AiChatTurn]>,
    transcript: &'a str,
}

#[derive(Debug, Deserialize)]
struct AiServerResponse {
    id: String,
    ok: bool,
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug)]
pub struct AiCommandRequest {
    pub command: &'static str,
    pub question: Option<String>,
    pub language: Option<String>,
    pub previous_summary: Option<String>,
    pub history: Vec<AiChatTurn>,
    pub transcript: String,
}

pub struct AiServer {
    child: Child,
    stdin: ChildStdin,
    pending: Arc<Mutex<HashMap<String, mpsc::Sender<AiServerResponse>>>>,
    next_id: u64,
}

enum AiRequestError {
    /// The response indicates a model/prompt problem; the server is healthy.
    Protocol(String),
    /// The pipe or process is broken; the server must be respawned.
    Transport(String),
}

impl AiServer {
    fn spawn() -> Result<AiServer, String> {
        let helper_path = resolve_helper_path()?;
        eprintln!(
            "live-poly-trans tauri: ai-server-start helper={}",
            helper_path.display()
        );

        let mut child = Command::new(helper_path)
            .arg("--ai-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "ai server stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "ai server stdout unavailable".to_string())?;
        let pending: Arc<Mutex<HashMap<String, mpsc::Sender<AiServerResponse>>>> = Arc::default();

        let reader_pending = pending.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(response) = serde_json::from_str::<AiServerResponse>(&line) {
                    if let Ok(mut guard) = reader_pending.lock() {
                        if let Some(sender) = guard.remove(&response.id) {
                            let _ = sender.send(response);
                        }
                    }
                }
            }
        });

        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    eprintln!("{line}");
                }
            });
        }

        Ok(AiServer {
            child,
            stdin,
            pending,
            next_id: 1,
        })
    }

    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn request(&mut self, request: &AiCommandRequest) -> Result<String, AiRequestError> {
        let id = self.next_id.to_string();
        self.next_id += 1;

        let payload = AiServerRequestPayload {
            id: id.clone(),
            command: request.command,
            question: request.question.as_deref(),
            language: request.language.as_deref(),
            previous_summary: request.previous_summary.as_deref(),
            history: if request.history.is_empty() {
                None
            } else {
                Some(&request.history)
            },
            transcript: &request.transcript,
        };
        let line = serde_json::to_string(&payload)
            .map_err(|error| AiRequestError::Transport(error.to_string()))?;

        let (sender, receiver) = mpsc::channel();
        if let Ok(mut guard) = self.pending.lock() {
            guard.insert(id.clone(), sender);
        }

        if let Err(error) = writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()) {
            if let Ok(mut guard) = self.pending.lock() {
                guard.remove(&id);
            }
            return Err(AiRequestError::Transport(error.to_string()));
        }

        match receiver.recv_timeout(AI_REQUEST_TIMEOUT) {
            Ok(response) => {
                if response.ok {
                    Ok(response.response.unwrap_or_default())
                } else {
                    Err(AiRequestError::Protocol(
                        response
                            .error
                            .unwrap_or_else(|| "unknown AI error".to_string()),
                    ))
                }
            }
            Err(_) => {
                if let Ok(mut guard) = self.pending.lock() {
                    guard.remove(&id);
                }
                Err(AiRequestError::Transport(
                    "AI request timed out".to_string(),
                ))
            }
        }
    }

    fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run_ai_request(
    ai_server: &Mutex<Option<AiServer>>,
    request: AiCommandRequest,
) -> Result<String, String> {
    let mut guard = ai_server.lock().map_err(|error| error.to_string())?;

    let needs_spawn = match guard.as_mut() {
        Some(server) => !server.is_alive(),
        None => true,
    };
    if needs_spawn {
        *guard = Some(AiServer::spawn()?);
    }

    let server = guard.as_mut().expect("ai server just ensured");
    match server.request(&request) {
        Ok(response) => Ok(response),
        Err(AiRequestError::Protocol(message)) => Err(message),
        Err(AiRequestError::Transport(message)) => {
            // Broken pipe or hang: kill and drop so the next call starts fresh.
            if let Some(mut server) = guard.take() {
                server.shutdown();
            }
            Err(message)
        }
    }
}

pub fn build_ai_context_from_saved_messages(messages: &[SavedTranscriptMessage]) -> String {
    messages
        .iter()
        .map(|message| {
            let translation = message
                .translation
                .as_ref()
                .filter(|translation| !translation.trim().is_empty())
                .map(|translation| format!("\n  => {translation}"))
                .unwrap_or_default();
            let speaker = message.speaker_label.as_deref().unwrap_or(&message.role);

            format!(
                "[{}] {} / {}: {}{}",
                message.timestamp, speaker, message.language, message.text, translation
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn action_timestamp_ms(message: &SavedTranscriptMessage) -> i64 {
    message
        .spans
        .as_ref()
        .and_then(|spans| spans.first())
        .and_then(|span| span.start_ms)
        .unwrap_or(0)
}

pub fn build_ai_action_context_from_saved_messages(messages: &[SavedTranscriptMessage]) -> String {
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let speaker = message.speaker_label.as_deref().unwrap_or(&message.role);
            format!(
                "[sourceIndex={index} timestampMs={}] {} / {}: {}",
                action_timestamp_ms(message),
                speaker,
                message.language,
                message.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// MARK: recordings

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrimRange {
    #[serde(rename = "sourceFile")]
    pub source_file: String,
    #[serde(rename = "startMs")]
    pub start_ms: i64,
    #[serde(rename = "endMs")]
    pub end_ms: i64,
}

/// Custom speaker identity, keyed by the frontend's stable speaker id
/// (`speaker-N` for diarized speakers, the stream name otherwise). Stored
/// only in meta.json — transcript jsonl is never rewritten; names resolve
/// at display/export time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerMeta {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingActionItem {
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(
        rename = "timestampMs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub timestamp_ms: Option<i64>,
    #[serde(
        rename = "sourceIndex",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub source_index: Option<i64>,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMeta {
    pub id: String,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "endedAt", default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// "external" for imported audio (phone/voice recorder files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Trim ranges by output file name, so the UI can shift the transcript
    /// to match a trimmed file's timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trims: Option<HashMap<String, TrimRange>>,
    /// Custom speaker names/colors by speaker id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speakers: Option<HashMap<String, SpeakerMeta>>,
    /// AI-extracted action items for this recording. Stored in meta.json so
    /// checkbox state can survive app restarts without rewriting transcript
    /// jsonl.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<RecordingActionItem>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordingSearchHit {
    #[serde(rename = "recordingId")]
    pub recording_id: String,
    #[serde(rename = "entryIndex")]
    pub entry_index: usize,
    pub snippet: String,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingFileInfo {
    pub name: String,
    pub stream: String,
    pub path: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingSummary {
    pub id: String,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "endedAt")]
    pub ended_at: Option<String>,
    pub source: Option<String>,
    pub trims: Option<HashMap<String, TrimRange>>,
    pub speakers: Option<HashMap<String, SpeakerMeta>>,
    pub actions: Option<Vec<RecordingActionItem>>,
    pub files: Vec<RecordingFileInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedRecording {
    pub id: String,
    pub dir: String,
}

pub fn readable_timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// "mic.m4a" / "mic-2.m4a" → "mic"; restart attempts keep their lane.
pub fn recording_stream_name(file_name: &str) -> String {
    let stem = file_name.split('.').next().unwrap_or(file_name);
    stem.split('-').next().unwrap_or(stem).to_string()
}

/// Picks `<stream>.m4a`, or a numbered variant when a crash-restart would
/// otherwise truncate the previous take.
pub fn next_recording_file_path(dir: &Path, stream: &str) -> PathBuf {
    let base = dir.join(format!("{stream}.m4a"));
    if !base.exists() {
        return base;
    }

    for attempt in 2..1000 {
        let candidate = dir.join(format!("{stream}-{attempt}.m4a"));
        if !candidate.exists() {
            return candidate;
        }
    }

    base
}

pub fn unique_destination(dir: &Path, base_name: &str, extension: &str) -> PathBuf {
    let first = dir.join(format!("{base_name}.{extension}"));
    if !first.exists() {
        return first;
    }

    for attempt in 2..1000 {
        let candidate = dir.join(format!("{base_name}-{attempt}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    first
}

/// Writes frontend-generated export text to a destination the user picked
/// in the save dialog; because the path is user-chosen no directory
/// allowlist applies, but an empty path is still a plain input error.
pub fn write_text_file(path: &str, contents: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("save path is empty".to_string());
    }

    fs::write(path, contents).map_err(|error| error.to_string())
}

pub fn validate_export_directory_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("export directory is empty".to_string());
    }
    if !path.is_dir() {
        return Err(format!(
            "export directory is not a directory: {}",
            path.display()
        ));
    }

    let probe = path.join(format!(
        ".live-poly-trans-write-test-{}",
        std::process::id()
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|error| format!("export directory is not writable: {error}"))?;
    drop(file);
    fs::remove_file(&probe).map_err(|error| error.to_string())
}

fn parse_segment_start_ms(segment_id: Option<&str>) -> i64 {
    segment_id
        .and_then(|value| value.split('-').next())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
}

fn snippet_around(text: &str, match_start: usize, match_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let start = match_start.saturating_sub(24);
    let end = (match_start + match_len + 24).min(chars.len());
    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.extend(chars[start..end].iter());
    if end < chars.len() {
        snippet.push('…');
    }
    snippet
}

pub fn search_transcript_events(
    recording_id: &str,
    query: &str,
    events: &[Value],
) -> Vec<RecordingSearchHit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut hits = Vec::new();
    let mut entry_index = 0usize;
    for event in events {
        let is_live_final = event.get("type").and_then(Value::as_str) == Some("transcript")
            && event.get("isFinal").and_then(Value::as_bool) == Some(true);
        let is_whisperx = event.get("type").and_then(Value::as_str) == Some("whisperx");
        if !is_live_final && !is_whisperx {
            continue;
        }

        let Some(text) = event.get("text").and_then(Value::as_str) else {
            continue;
        };
        let haystack = text.to_lowercase();
        if let Some(byte_index) = haystack.find(&needle) {
            let match_start = haystack[..byte_index].chars().count();
            let match_len = needle.chars().count();
            let timestamp_ms = event
                .get("startMs")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| {
                    parse_segment_start_ms(event.get("segmentId").and_then(Value::as_str))
                });
            hits.push(RecordingSearchHit {
                recording_id: recording_id.to_string(),
                entry_index,
                snippet: snippet_around(text, match_start, match_len),
                timestamp_ms,
            });
        }
        entry_index += 1;
    }

    hits
}

pub fn read_recording_transcript_events(dir: &Path) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut events = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }

        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };

        for line in contents.lines() {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                events.push(value);
            }
        }
    }

    events
}

pub fn search_recordings_in_root(
    root: &Path,
    query: &str,
) -> Result<Vec<RecordingSearchHit>, String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(Vec::new());
    };

    let mut hits = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }

        let Some(meta) = read_recording_meta(&dir) else {
            continue;
        };
        let events = read_recording_transcript_events(&dir);
        hits.extend(search_transcript_events(&meta.id, query, &events));
    }

    Ok(hits)
}

fn recordings_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("recordings"))
}

fn recording_dir_for(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if id.contains('/') || id.contains("..") {
        return Err(format!("invalid recording id: {id}"));
    }

    Ok(recordings_dir(app)?.join(id))
}

fn read_recording_meta(dir: &Path) -> Option<RecordingMeta> {
    let raw = fs::read_to_string(dir.join("meta.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_recording_meta(dir: &Path, meta: &RecordingMeta) -> Result<(), String> {
    let json = serde_json::to_string_pretty(meta).map_err(|error| error.to_string())?;
    fs::write(dir.join("meta.json"), json).map_err(|error| error.to_string())
}

/// Replaces the recording's speaker map in meta.json wholesale; an empty
/// map removes the key so untouched recordings keep a clean meta file.
pub fn update_recording_speakers_in_dir(
    dir: &Path,
    speakers: HashMap<String, SpeakerMeta>,
) -> Result<(), String> {
    let mut meta = read_recording_meta(dir)
        .ok_or_else(|| format!("recording meta not found in {}", dir.display()))?;
    meta.speakers = if speakers.is_empty() {
        None
    } else {
        Some(speakers)
    };
    write_recording_meta(dir, &meta)
}

/// Replaces AI action items in meta.json wholesale. An empty list removes the
/// key so recordings with no actionable follow-up keep compact metadata.
pub fn update_recording_actions_in_dir(
    dir: &Path,
    actions: Vec<RecordingActionItem>,
) -> Result<(), String> {
    let mut meta = read_recording_meta(dir)
        .ok_or_else(|| format!("recording meta not found in {}", dir.display()))?;
    meta.actions = if actions.is_empty() {
        None
    } else {
        Some(actions)
    };
    write_recording_meta(dir, &meta)
}

// MARK: whisperx post-processing

/// How to invoke WhisperX. Spike outcome (plan 6-1): prefer `uvx` (ships
/// with uv, runs the tool without a permanent install), then `pipx run`,
/// then a `whisperx` binary already on PATH. Diarization needs a pyannote
/// HF token; without one we run transcription+alignment only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperxRunner {
    pub program: String,
    pub prefix_args: Vec<String>,
}

pub fn whisperx_runner_from(available: impl Fn(&str) -> Option<String>) -> Option<WhisperxRunner> {
    if let Some(program) = available("uvx") {
        return Some(WhisperxRunner {
            program,
            prefix_args: vec!["whisperx".to_string()],
        });
    }

    if let Some(program) = available("pipx") {
        return Some(WhisperxRunner {
            program,
            prefix_args: vec!["run".to_string(), "whisperx".to_string()],
        });
    }

    if let Some(program) = available("whisperx") {
        return Some(WhisperxRunner {
            program,
            prefix_args: Vec::new(),
        });
    }

    None
}

/// Where a program may live, most specific first. The app-managed directory
/// leads so a uv we downloaded ourselves wins over an older Homebrew one.
pub fn program_candidates(name: &str, home: &str, managed_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = managed_dir.map(|dir| dir.join(name)).into_iter().collect();

    candidates.extend(
        [
            format!("/opt/homebrew/bin/{name}"),
            format!("/usr/local/bin/{name}"),
            format!("{home}/.local/bin/{name}"),
            format!("{home}/.cargo/bin/{name}"),
        ]
        .map(PathBuf::from),
    );

    candidates
}

pub fn locate_program(name: &str, managed_dir: Option<&Path>) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();

    for candidate in program_candidates(name, &home, managed_dir) {
        if candidate.exists() {
            return Some(candidate.display().to_string());
        }
    }

    let found = Command::new("/usr/bin/which")
        .arg(name)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|path| !path.is_empty());

    found
}

pub fn resolve_whisperx_runner(managed_uv_dir: Option<&Path>) -> Option<WhisperxRunner> {
    whisperx_runner_from(|name| locate_program(name, managed_uv_dir))
}

// MARK: uv provisioning (the WhisperX runtime)

/// The uv release we install ourselves. Pinning the version lets us keep the
/// checksum in source, so the archive is verified without trusting the same
/// network that served it; a bump is a deliberate code change.
pub const UV_VERSION: &str = "0.12.1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UvDownload {
    pub url: String,
    pub sha256: &'static str,
    pub archive_dir: String,
}

pub fn uv_download_for(arch: &str) -> Option<UvDownload> {
    let sha256 = match arch {
        "aarch64" => "77d2906988e8074fd43f2f329ec452ebbf9b0c257ba1c66451c71de70a6baf42",
        "x86_64" => "69d9f9a00337f25a50dcb13882052da08b8469bac11091c98c5694c3c6721467",
        _ => return None,
    };

    let archive_dir = format!("uv-{arch}-apple-darwin");
    Some(UvDownload {
        url: format!(
            "https://github.com/astral-sh/uv/releases/download/{UV_VERSION}/{archive_dir}.tar.gz"
        ),
        sha256,
        archive_dir,
    })
}

pub fn current_uv_download() -> Option<UvDownload> {
    uv_download_for(if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    })
}

pub fn managed_uv_dir(app_data: &Path) -> PathBuf {
    app_data.join("tools").join("uv")
}

pub fn sha256_from_shasum_output(output: &str) -> Option<String> {
    let digest = output.split_whitespace().next()?;
    (digest.len() == 64 && digest.chars().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| digest.to_ascii_lowercase())
}

/// Fetches uv into `dest_dir` using only tools macOS already ships, so the
/// app gains no HTTP/archive/hash dependencies. `run` is injected to keep the
/// sequence (and the refusal to extract an archive that failed verification)
/// testable. Both directories must exist; this performs no filesystem work of
/// its own.
pub fn install_uv_with(
    download: &UvDownload,
    work_dir: &Path,
    dest_dir: &Path,
    run: impl Fn(&str, &[String]) -> Result<String, String>,
    report: impl Fn(&str),
) -> Result<(), String> {
    let archive = work_dir.join("uv.tar.gz").display().to_string();

    report("download");
    run(
        "/usr/bin/curl",
        &[
            "-fsSL".to_string(),
            "--retry".to_string(),
            "3".to_string(),
            "-o".to_string(),
            archive.clone(),
            download.url.clone(),
        ],
    )?;

    report("verify");
    let shasum = run(
        "/usr/bin/shasum",
        &["-a".to_string(), "256".to_string(), archive.clone()],
    )?;
    let digest = sha256_from_shasum_output(&shasum).ok_or_else(|| {
        "ダウンロードしたファイルのチェックサムを読み取れませんでした。".to_string()
    })?;
    if digest != download.sha256 {
        return Err(format!(
            "ダウンロードしたファイルのチェックサムが一致しません(期待 {} / 実際 {})。",
            download.sha256, digest
        ));
    }

    // --strip-components drops the uv-<arch>-apple-darwin/ wrapper so uv and
    // uvx land directly in dest_dir; the archive already carries +x.
    report("extract");
    run(
        "/usr/bin/tar",
        &[
            "-xzf".to_string(),
            archive,
            "--strip-components".to_string(),
            "1".to_string(),
            "-C".to_string(),
            dest_dir.display().to_string(),
        ],
    )?;

    report("done");
    Ok(())
}

pub fn run_command(program: &str, args: &[String]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("{program} を起動できませんでした: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "{program} が失敗しました: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// MARK: privacy permissions

/// Deep links into the exact System Settings pane. Once a permission has been
/// requested at least once the app is already listed there, so this turns a
/// denial into a single checkbox instead of a hunt.
pub fn privacy_settings_url(kind: &str) -> Option<&'static str> {
    match kind {
        "microphone" => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        }
        "screen-recording" => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        }
        _ => None,
    }
}

/// Maps whisperx stderr progress markers onto the three UI stages.
pub fn whisperx_stage_for_line(line: &str) -> Option<&'static str> {
    let lowered = line.to_lowercase();
    if lowered.contains("performing transcription") {
        Some("transcribe")
    } else if lowered.contains("performing alignment") {
        Some("align")
    } else if lowered.contains("performing diarization") {
        Some("diarize")
    } else {
        None
    }
}

pub const WHISPERX_REPROCESS_MODEL: &str = "large-v3-turbo";

pub fn whisperx_reprocess_args(
    input: &Path,
    output_dir: &Path,
    hf_token: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        input.display().to_string(),
        "--output_format".to_string(),
        "json".to_string(),
        "--output_dir".to_string(),
        output_dir.display().to_string(),
        "--compute_type".to_string(),
        "int8".to_string(),
        "--model".to_string(),
        WHISPERX_REPROCESS_MODEL.to_string(),
        "--no_align".to_string(),
    ];

    if let Some(token) = hf_token.filter(|token| !token.trim().is_empty()) {
        args.push("--diarize".to_string());
        args.push("--hf_token".to_string());
        args.push(token.to_string());
    }

    args
}

/// Converts whisperx JSON output into our transcript.whisperx.jsonl lines.
/// Times become integer milliseconds; the speaker tag is passed through
/// (SPEAKER_00, ...) and mapped to labels/colors in the UI.
pub fn whisperx_jsonl_lines(output: &Value) -> Vec<String> {
    let language = output
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("und");
    let Some(segments) = output.get("segments").and_then(Value::as_array) else {
        return Vec::new();
    };

    segments
        .iter()
        .filter_map(|segment| {
            let text = segment.get("text")?.as_str()?.trim();
            if text.is_empty() {
                return None;
            }

            let start = segment.get("start").and_then(Value::as_f64).unwrap_or(0.0);
            let end = segment.get("end").and_then(Value::as_f64).unwrap_or(start);
            let mut line = serde_json::json!({
                "type": "whisperx",
                "startMs": (start * 1000.0).round() as i64,
                "endMs": (end * 1000.0).round() as i64,
                "text": text,
                "lang": language,
            });
            if let Some(speaker) = segment.get("speaker").and_then(Value::as_str) {
                line["speaker"] = Value::String(speaker.to_string());
            }
            Some(line.to_string())
        })
        .collect()
}

// MARK: recording session control

pub fn start_recording_control_line(dir: &str, include_audio: bool) -> String {
    serde_json::json!({ "cmd": "start-recording", "dir": dir, "audio": include_audio }).to_string()
}

pub fn stop_recording_control_line() -> String {
    serde_json::json!({ "cmd": "stop-recording" }).to_string()
}

/// Writes one control line to every running helper's stdin. Ok carries how
/// many helpers got the line; Err lists the streams whose write failed.
pub fn send_control_line_to_children(
    children: &Mutex<HashMap<String, StreamChild>>,
    line: &str,
) -> Result<usize, String> {
    let mut guard = children.lock().map_err(|error| error.to_string())?;
    let mut written = 0;
    let mut failures = Vec::new();

    for (stream, entry) in guard.iter_mut() {
        let Some(stdin) = entry.child.stdin.as_mut() else {
            failures.push(format!("{stream}: stdin unavailable"));
            continue;
        };

        match writeln!(stdin, "{line}").and_then(|_| stdin.flush()) {
            Ok(()) => written += 1,
            Err(error) => failures.push(format!("{stream}: {error}")),
        }
    }

    if failures.is_empty() {
        Ok(written)
    } else {
        Err(failures.join("\n"))
    }
}

/// Blocks until every running helper has announced its stdin control
/// channel. Distinguishes "helper still starting up" (wait) from "helper
/// binary predates the control channel" (times out with a clear message).
fn wait_for_control_ready(
    children: &Mutex<HashMap<String, StreamChild>>,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;

    loop {
        let pending: Vec<String> = {
            let guard = children.lock().map_err(|error| error.to_string())?;
            if guard.is_empty() {
                return Err("no capture is running; recording needs live transcription".to_string());
            }

            guard
                .iter()
                .filter(|(_, entry)| !entry.control_ready.load(Ordering::Relaxed))
                .map(|(stream, _)| stream.clone())
                .collect()
        };

        if pending.is_empty() {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "the running helper does not accept recording control ({}); the helper binary is outdated — rebuild it with `bun run build:helper`",
                pending.join(", ")
            ));
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Deletes one recording directory (audio, transcript, meta). The id is
/// re-validated here because this is the only command that removes files.
pub fn delete_recording_dir(root: &Path, id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains('/') || id.contains("..") {
        return Err(format!("invalid recording id: {id}"));
    }

    let dir = root.join(id);
    if !dir.exists() {
        return Ok(());
    }

    fs::remove_dir_all(&dir).map_err(|error| error.to_string())
}

/// Closes every recording still marked in-progress. Quitting the app while
/// recording never reaches `finalize_recording`, which would leave the
/// recording open forever and orphaned by the next start.
pub fn close_open_recordings(root: &Path, ended_at: &str) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(mut meta) = read_recording_meta(&dir) else {
            continue;
        };
        if meta.ended_at.is_none() {
            meta.ended_at = Some(ended_at.to_string());
            let _ = write_recording_meta(&dir, &meta);
        }
    }
}

// MARK: commands

pub mod commands {
    use super::*;

    /// Non-async commands run on the main thread in Tauri v2; this one blocks
    /// on a helper subprocess, so it must not.
    #[tauri::command]
    pub async fn detect_languages() -> Result<LanguageDetectionPayload, String> {
        tauri::async_runtime::spawn_blocking(|| {
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
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Runs a helper language-pack command (`--install-language` /
    /// `--uninstall-language`) and returns the refreshed detection payload.
    /// Installation blocks while the speech model downloads, so it runs on a
    /// blocking thread instead of the async runtime.
    async fn run_language_pack_command(
        flag: &'static str,
        language: String,
    ) -> Result<LanguageDetectionPayload, String> {
        tauri::async_runtime::spawn_blocking(move || {
            let helper_path = resolve_helper_path()?;
            eprintln!(
                "live-poly-trans tauri: language-pack flag={flag} language={language} helper={}",
                helper_path.display()
            );

            let output = Command::new(helper_path)
                .arg(flag)
                .arg(&language)
                .output()
                .map_err(|error| error.to_string())?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
            }

            serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn install_language(language: String) -> Result<LanguageDetectionPayload, String> {
        run_language_pack_command("--install-language", language).await
    }

    #[tauri::command]
    pub async fn uninstall_language(language: String) -> Result<LanguageDetectionPayload, String> {
        run_language_pack_command("--uninstall-language", language).await
    }

    #[allow(clippy::too_many_arguments)]
    #[tauri::command]
    pub async fn start_stream_session(
        app: AppHandle,
        state: State<'_, HelperSession>,
        stream: String,
        source_language: String,
        target_language: String,
        languages: Vec<String>,
        session_id: String,
        recording_dir: Option<String>,
        recording_audio: Option<bool>,
        engine: Option<String>,
        whisper_model: Option<String>,
    ) -> Result<(), String> {
        let children = state.children.clone();

        tauri::async_runtime::spawn_blocking(move || {
            let whisper_config =
                resolve_whisper_config(engine.as_deref(), whisper_model.as_deref())?;
            let stop_grace = stop_grace_for_engine(engine.as_deref());

            stop_stream_child(&children, &stream)?;

            let segment_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?
                .join("segments")
                .join(&stream);
            fs::create_dir_all(&segment_dir).map_err(|error| error.to_string())?;

            let (record_file, transcript_file) = match recording_dir.as_deref() {
                Some(dir) => {
                    let dir = PathBuf::from(dir);
                    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
                    (
                        // The transcript is always part of a recording; audio
                        // is optional (settings: 録音に音声ファイルを含める).
                        if recording_audio.unwrap_or(true) {
                            Some(
                                next_recording_file_path(&dir, &stream)
                                    .to_string_lossy()
                                    .into_owned(),
                            )
                        } else {
                            None
                        },
                        Some(
                            dir.join(format!("{stream}.jsonl"))
                                .to_string_lossy()
                                .into_owned(),
                        ),
                    )
                }
                None => (None, None),
            };

            let helper_args = build_stream_helper_args(
                &stream,
                &source_language,
                &target_language,
                &languages,
                &segment_dir.to_string_lossy(),
                record_file.as_deref(),
                transcript_file.as_deref(),
                whisper_config
                    .as_ref()
                    .map(WhisperEngineConfig::from)
                    .as_ref(),
            );

            let helper_path = resolve_helper_path()?;
            eprintln!(
                "live-poly-trans tauri: start-stream stream={} helper={} args={:?}",
                stream,
                helper_path.display(),
                helper_args
            );

            let mut child = Command::new(helper_path)
                .args(helper_args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| error.to_string())?;
            eprintln!(
                "live-poly-trans tauri: helper-started stream={} pid={}",
                stream,
                child.id()
            );

            let control_ready = Arc::new(AtomicBool::new(false));
            if let Some(stdout) = child.stdout.take() {
                read_json_lines(
                    app.clone(),
                    stdout,
                    session_id.clone(),
                    control_ready.clone(),
                );
            }

            if let Some(stderr) = child.stderr.take() {
                read_stderr(app.clone(), stderr);
            }

            children.lock().map_err(|error| error.to_string())?.insert(
                stream.clone(),
                StreamChild {
                    session_id: session_id.clone(),
                    child,
                    stop_grace,
                    control_ready,
                },
            );

            watch_helper_exit(app, children.clone(), stream, session_id);
            Ok(())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn list_speech_models(app: AppHandle) -> Result<SpeechModelsPayload, String> {
        tauri::async_runtime::spawn_blocking(move || {
            let home = app.path().home_dir().map_err(|error| error.to_string())?;

            Ok(SpeechModelsPayload {
                models: scan_speech_models(&superwhisper_models_dir(&home)),
                cli_available: resolve_whisper_cli().is_ok(),
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn stop_stream_session(
        state: State<'_, HelperSession>,
        stream: String,
    ) -> Result<(), String> {
        let children = state.children.clone();
        tauri::async_runtime::spawn_blocking(move || stop_stream_child(&children, &stream))
            .await
            .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn stop_all_sessions(state: State<'_, HelperSession>) -> Result<(), String> {
        let children = state.children.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let streams = children
                .lock()
                .map_err(|error| error.to_string())?
                .keys()
                .cloned()
                .collect::<Vec<_>>();

            stop_each_stream(&streams, |stream| stop_stream_child(&children, stream))
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn save_transcript(
        app: AppHandle,
        messages: Vec<SavedTranscriptMessage>,
    ) -> Result<SaveTranscriptResult, String> {
        tauri::async_runtime::spawn_blocking(move || save_transcript_blocking(&app, &messages))
            .await
            .map_err(|error| error.to_string())?
    }

    fn save_transcript_blocking(
        app: &AppHandle,
        messages: &[SavedTranscriptMessage],
    ) -> Result<SaveTranscriptResult, String> {
        // Downloads, not app data: ~/Library is invisible in Finder by
        // default, so saving there reads as data loss to most users.
        let transcript_dir = app
            .path()
            .download_dir()
            .map_err(|error| error.to_string())?;
        fs::create_dir_all(&transcript_dir).map_err(|error| error.to_string())?;

        let timestamp = readable_timestamp();
        let base_name = format!("LivePolyTrans-transcript-{timestamp}");
        let json_path = unique_destination(&transcript_dir, &base_name, "json");
        let text_path = unique_destination(&transcript_dir, &base_name, "txt");

        let json = serde_json::to_string_pretty(messages).map_err(|error| error.to_string())?;
        fs::write(&json_path, json).map_err(|error| error.to_string())?;

        let mut text_file = File::create(&text_path).map_err(|error| error.to_string())?;
        for message in messages {
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

    #[tauri::command]
    pub async fn ai_generate_summary(
        state: State<'_, HelperSession>,
        messages: Vec<SavedTranscriptMessage>,
        source_language: String,
        previous_summary: Option<String>,
    ) -> Result<String, String> {
        let ai_server = state.ai_server.clone();
        let request = AiCommandRequest {
            command: "summary",
            question: None,
            language: Some(source_language),
            previous_summary,
            history: Vec::new(),
            transcript: build_ai_context_from_saved_messages(&messages),
        };

        tauri::async_runtime::spawn_blocking(move || run_ai_request(&ai_server, request))
            .await
            .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn ai_suggest_questions(
        state: State<'_, HelperSession>,
        messages: Vec<SavedTranscriptMessage>,
        language: Option<String>,
    ) -> Result<String, String> {
        let ai_server = state.ai_server.clone();
        let request = AiCommandRequest {
            command: "suggest",
            question: None,
            language,
            previous_summary: None,
            history: Vec::new(),
            transcript: build_ai_context_from_saved_messages(&messages),
        };

        tauri::async_runtime::spawn_blocking(move || run_ai_request(&ai_server, request))
            .await
            .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn ai_extract_actions(
        state: State<'_, HelperSession>,
        messages: Vec<SavedTranscriptMessage>,
        language: Option<String>,
    ) -> Result<String, String> {
        let ai_server = state.ai_server.clone();
        let request = AiCommandRequest {
            command: "actions",
            question: None,
            language,
            previous_summary: None,
            history: Vec::new(),
            transcript: build_ai_action_context_from_saved_messages(&messages),
        };

        tauri::async_runtime::spawn_blocking(move || run_ai_request(&ai_server, request))
            .await
            .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn toggle_overlay(app: AppHandle) -> Result<bool, String> {
        if let Some(window) = app.get_webview_window("overlay") {
            let visible = window.is_visible().map_err(|error| error.to_string())?;
            if visible {
                if let Ok(path) = overlay_window_state_path(&app) {
                    if let Ok(state) = capture_overlay_window_state(&window) {
                        let _ = write_overlay_window_state(&path, &state);
                    }
                }
                window.hide().map_err(|error| error.to_string())?;
                return Ok(false);
            }

            window.show().map_err(|error| error.to_string())?;
            if let Some(state) = overlay_window_state_path(&app)
                .ok()
                .and_then(|path| read_overlay_window_state(&path))
            {
                let _ = apply_overlay_window_state(&window, &state);
            }
            window
                .set_ignore_cursor_events(true)
                .map_err(|error| error.to_string())?;
            return Ok(true);
        }

        let window = WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::App("overlay".into()))
            .title("LivePolyTrans Overlay")
            .inner_size(980.0, 180.0)
            .min_inner_size(360.0, 96.0)
            .decorations(false)
            .background_color(tauri::utils::config::Color(0, 0, 0, 0))
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .skip_taskbar(true)
            .resizable(true)
            .build()
            .map_err(|error| error.to_string())?;
        if let Some(state) = overlay_window_state_path(&app)
            .ok()
            .and_then(|path| read_overlay_window_state(&path))
        {
            let _ = apply_overlay_window_state(&window, &state);
        }
        window
            .set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    #[tauri::command]
    pub async fn begin_overlay_adjustment(
        app: AppHandle,
        state: State<'_, OverlayAdjustmentState>,
    ) -> Result<(), String> {
        if app.get_webview_window("overlay").is_none() {
            toggle_overlay(app.clone()).await?;
        }

        let window = app
            .get_webview_window("overlay")
            .ok_or_else(|| "overlay window is not available".to_string())?;
        *state.0.lock().map_err(|error| error.to_string())? = true;
        window.show().map_err(|error| error.to_string())?;
        window
            .set_ignore_cursor_events(false)
            .map_err(|error| error.to_string())?;
        window
            .emit("overlay-adjustment", true)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub async fn start_overlay_drag(app: AppHandle) -> Result<(), String> {
        let window = app
            .get_webview_window("overlay")
            .ok_or_else(|| "overlay window is not available".to_string())?;
        window.start_dragging().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub async fn finish_overlay_adjustment(
        app: AppHandle,
        adjustment_state: State<'_, OverlayAdjustmentState>,
    ) -> Result<(), String> {
        let window = app
            .get_webview_window("overlay")
            .ok_or_else(|| "overlay window is not available".to_string())?;
        let window_state = capture_overlay_window_state(&window)?;
        write_overlay_window_state(&overlay_window_state_path(&app)?, &window_state)?;
        *adjustment_state
            .0
            .lock()
            .map_err(|error| error.to_string())? = false;
        window
            .set_ignore_cursor_events(true)
            .map_err(|error| error.to_string())?;
        window
            .emit("overlay-adjustment", false)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub async fn overlay_adjustment_enabled(
        state: State<'_, OverlayAdjustmentState>,
    ) -> Result<bool, String> {
        Ok(*state.0.lock().map_err(|error| error.to_string())?)
    }

    #[tauri::command]
    pub async fn configure_global_shortcuts(
        app: AppHandle,
        enabled: bool,
        recording_shortcut: String,
        overlay_shortcut: String,
    ) -> Result<(), String> {
        configure_global_shortcuts_for_app(&app, enabled, &recording_shortcut, &overlay_shortcut)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub async fn ai_ask(
        state: State<'_, HelperSession>,
        question: String,
        messages: Vec<SavedTranscriptMessage>,
        language: Option<String>,
        history: Option<Vec<AiChatTurn>>,
    ) -> Result<String, String> {
        let ai_server = state.ai_server.clone();
        let request = AiCommandRequest {
            command: "ask",
            question: Some(question),
            language,
            previous_summary: None,
            history: history.unwrap_or_default(),
            transcript: build_ai_context_from_saved_messages(&messages),
        };

        tauri::async_runtime::spawn_blocking(move || run_ai_request(&ai_server, request))
            .await
            .map_err(|error| error.to_string())?
    }

    fn create_recording_blocking(app: &AppHandle) -> Result<CreatedRecording, String> {
        let id = format!("rec-{}", readable_timestamp());
        let dir = recording_dir_for(app, &id)?;
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;

        write_recording_meta(
            &dir,
            &RecordingMeta {
                id: id.clone(),
                started_at: chrono::Local::now().to_rfc3339(),
                ended_at: None,
                source: None,
                trims: None,
                speakers: None,
                actions: None,
            },
        )?;

        Ok(CreatedRecording {
            id,
            dir: dir.display().to_string(),
        })
    }

    fn finalize_recording_blocking(app: &AppHandle, id: &str) -> Result<(), String> {
        let dir = recording_dir_for(app, id)?;
        let Some(mut meta) = read_recording_meta(&dir) else {
            return Ok(());
        };

        meta.ended_at = Some(chrono::Local::now().to_rfc3339());
        write_recording_meta(&dir, &meta)
    }

    #[tauri::command]
    pub async fn create_recording(app: AppHandle) -> Result<CreatedRecording, String> {
        tauri::async_runtime::spawn_blocking(move || create_recording_blocking(&app))
            .await
            .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn finalize_recording(app: AppHandle, id: String) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || finalize_recording_blocking(&app, &id))
            .await
            .map_err(|error| error.to_string())?
    }

    /// Opens a recording session against the already-running capture: creates
    /// the recording directory, then tells every live helper to start writing
    /// audio + transcript into it. Capture is never restarted.
    #[tauri::command]
    pub async fn start_recording_session(
        app: AppHandle,
        state: State<'_, HelperSession>,
        include_audio: Option<bool>,
    ) -> Result<CreatedRecording, String> {
        let children = state.children.clone();

        tauri::async_runtime::spawn_blocking(move || {
            wait_for_control_ready(&children, CONTROL_READY_TIMEOUT)?;
            let created = create_recording_blocking(&app)?;

            let line = start_recording_control_line(&created.dir, include_audio.unwrap_or(true));
            if let Err(error) = send_control_line_to_children(&children, &line) {
                // The dir stays on disk (it may already hold partial data from
                // helpers that did get the line); mark it closed so it is not
                // treated as in-progress forever.
                let _ = finalize_recording_blocking(&app, &created.id);
                return Err(format!("could not start recording: {error}"));
            }

            eprintln!(
                "live-poly-trans tauri: recording-session-start id={} dir={}",
                created.id, created.dir
            );
            Ok(created)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Closes a recording session: helpers detach their recorders (keeping
    /// capture running) and the meta gets its end timestamp. Helper write
    /// failures are logged but do not fail the stop — a crashed helper's
    /// audio is already on disk and the meta must still be closed.
    #[tauri::command]
    pub async fn stop_recording_session(
        app: AppHandle,
        state: State<'_, HelperSession>,
        id: String,
    ) -> Result<(), String> {
        let children = state.children.clone();

        tauri::async_runtime::spawn_blocking(move || {
            if let Err(error) =
                send_control_line_to_children(&children, &stop_recording_control_line())
            {
                eprintln!("live-poly-trans tauri: recording-session-stop-warning {error}");
            }

            eprintln!("live-poly-trans tauri: recording-session-stop id={id}");
            finalize_recording_blocking(&app, &id)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn recordings_directory(app: AppHandle) -> Result<String, String> {
        Ok(recordings_dir(&app)?.display().to_string())
    }

    /// Opens the recordings folder in Finder (settings: 保存先を表示).
    #[tauri::command]
    pub async fn reveal_recordings_directory(app: AppHandle) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || {
            let dir = recordings_dir(&app)?;
            fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            Command::new("open")
                .arg(&dir)
                .spawn()
                .map_err(|error| error.to_string())?;
            Ok(())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn delete_recording(app: AppHandle, id: String) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || {
            let root = recordings_dir(&app)?;
            delete_recording_dir(&root, &id)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    fn managed_uv_dir_for(app: &AppHandle) -> Result<PathBuf, String> {
        Ok(managed_uv_dir(
            &app.path().app_data_dir().map_err(|e| e.to_string())?,
        ))
    }

    /// Reports how WhisperX would be run (settings guidance); None when no
    /// runner is installed.
    #[tauri::command]
    pub async fn whisperx_status(app: AppHandle) -> Result<Option<WhisperxRunner>, String> {
        tauri::async_runtime::spawn_blocking(move || {
            let managed = managed_uv_dir_for(&app)?;
            Ok(resolve_whisperx_runner(Some(&managed)))
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Installs uv into the app's own data directory when nothing usable is on
    /// the machine, so WhisperX no longer requires `brew install uv` first.
    /// A runner that already exists (Homebrew, pipx, a previous install) is
    /// returned untouched.
    #[tauri::command]
    pub async fn ensure_uv(app: AppHandle) -> Result<WhisperxRunner, String> {
        tauri::async_runtime::spawn_blocking(move || {
            let managed = managed_uv_dir_for(&app)?;
            if let Some(runner) = resolve_whisperx_runner(Some(&managed)) {
                return Ok(runner);
            }

            let download = current_uv_download()
                .ok_or_else(|| "このアーキテクチャ向けの uv 配布物がありません。".to_string())?;
            let work = std::env::temp_dir().join("live-poly-trans-uv");

            fs::create_dir_all(&work).map_err(|error| error.to_string())?;
            fs::create_dir_all(&managed).map_err(|error| error.to_string())?;

            let result = install_uv_with(&download, &work, &managed, run_command, |stage| {
                let _ = app.emit("uv-install-progress", stage);
            });
            let _ = fs::remove_dir_all(&work);
            result?;

            resolve_whisperx_runner(Some(&managed))
                .ok_or_else(|| "uv を展開しましたが uvx が見つかりませんでした。".to_string())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Reads the privacy permissions without prompting, so the app can decide
    /// whether to auto-start instead of firing system dialogs at launch.
    #[tauri::command]
    pub async fn permission_status() -> Result<Value, String> {
        tauri::async_runtime::spawn_blocking(|| {
            let helper_path = resolve_helper_path()?;
            let output = run_command(
                &helper_path.display().to_string(),
                &["--check-permissions".to_string()],
            )?;
            serde_json::from_str(output.trim()).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Requests exactly one permission, on an explicit user action. macOS only
    /// lists an app under Privacy & Security once it has asked at least once,
    /// so this is also what puts us in the list the user ticks.
    #[tauri::command]
    pub async fn request_permission(kind: String) -> Result<Value, String> {
        tauri::async_runtime::spawn_blocking(move || {
            let helper_path = resolve_helper_path()?;
            let output = run_command(
                &helper_path.display().to_string(),
                &["--request-permission".to_string(), kind],
            )?;
            serde_json::from_str(output.trim()).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn open_privacy_settings(kind: String) -> Result<(), String> {
        let url = privacy_settings_url(&kind)
            .ok_or_else(|| format!("unknown permission kind: {kind}"))?;
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Picks which audio file a re-process should read: the mixed file when
    /// both lanes exist (rebuilding it if stale), else the first audio file.
    fn resolve_reprocess_input(dir: &Path) -> Result<PathBuf, String> {
        let mic = dir.join("mic.m4a");
        let speaker = dir.join("speaker.m4a");

        if mic.exists() && speaker.exists() {
            let mixed = dir.join("mixed.m4a");
            let is_stale = match (fs::metadata(&mixed), fs::metadata(&mic)) {
                (Ok(mixed_meta), Ok(mic_meta)) => {
                    matches!(
                        (mixed_meta.modified(), mic_meta.modified()),
                        (Ok(mixed_time), Ok(mic_time)) if mixed_time < mic_time
                    )
                }
                _ => true,
            };

            if is_stale {
                let helper_path = resolve_helper_path()?;
                let output = Command::new(helper_path)
                    .arg("--mix")
                    .arg("--input")
                    .arg(&mic)
                    .arg("--input")
                    .arg(&speaker)
                    .arg("--output")
                    .arg(&mixed)
                    .output()
                    .map_err(|error| error.to_string())?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
                }
            }
            return Ok(mixed);
        }

        let entries = fs::read_dir(dir).map_err(|error| error.to_string())?;
        let mut audio_files: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("m4a" | "wav" | "mp3" | "aac")
                )
            })
            .collect();
        audio_files.sort();
        audio_files
            .into_iter()
            .next()
            .ok_or_else(|| "この録音に音声ファイルがありません".to_string())
    }

    /// Runs WhisperX over a recording's audio and writes the result next to
    /// the live transcript as transcript.whisperx.jsonl (non-destructive).
    /// Progress is emitted as reprocess-progress events per stage.
    #[tauri::command]
    pub async fn reprocess_recording(
        app: AppHandle,
        id: String,
        engine: String,
        hf_token: Option<String>,
    ) -> Result<usize, String> {
        if engine != "whisperx" {
            return Err(format!(
                "engine {engine} の再処理は未対応です(現在は whisperx のみ)"
            ));
        }

        let dir = recording_dir_for(&app, &id)?;

        tauri::async_runtime::spawn_blocking(move || {
            let managed = managed_uv_dir_for(&app)?;
            let runner = resolve_whisperx_runner(Some(&managed)).ok_or_else(|| {
                "WhisperX の実行環境が見つかりません。設定 > 認識モデル の「WhisperX を準備する」から実行環境を用意してください。".to_string()
            })?;

            let emit_stage = |stage: &str| {
                let _ = app.emit(
                    "reprocess-progress",
                    serde_json::json!({ "id": id, "stage": stage }),
                );
            };

            let input = resolve_reprocess_input(&dir)?;
            let output_dir = dir.join("whisperx-tmp");
            fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;

            let token = hf_token.filter(|token| !token.trim().is_empty());
            let mut args = runner.prefix_args.clone();
            args.extend(whisperx_reprocess_args(&input, &output_dir, token.as_deref()));

            eprintln!(
                "live-poly-trans tauri: reprocess-start id={id} runner={} model={} no_align=true diarize={}",
                runner.program,
                WHISPERX_REPROCESS_MODEL,
                token.is_some()
            );
            emit_stage("transcribe");

            let mut child = Command::new(&runner.program)
                .args(&args)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| format!("WhisperX を起動できませんでした: {error}"))?;

            let mut stderr_tail: Vec<String> = Vec::new();
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(stage) = whisperx_stage_for_line(&line) {
                        emit_stage(stage);
                    }
                    stderr_tail.push(line);
                    if stderr_tail.len() > 40 {
                        stderr_tail.remove(0);
                    }
                }
            }

            let status = child.wait().map_err(|error| error.to_string())?;
            if !status.success() {
                let _ = fs::remove_dir_all(&output_dir);
                return Err(format!(
                    "WhisperX が失敗しました:\n{}",
                    stderr_tail.join("\n")
                ));
            }

            let stem = input
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let json_path = output_dir.join(format!("{stem}.json"));
            let raw = fs::read_to_string(&json_path)
                .map_err(|error| format!("WhisperX の出力を読めませんでした: {error}"))?;
            let parsed: Value =
                serde_json::from_str(&raw).map_err(|error| error.to_string())?;
            let lines = whisperx_jsonl_lines(&parsed);

            fs::write(
                dir.join("transcript.whisperx.jsonl"),
                format!("{}\n", lines.join("\n")),
            )
            .map_err(|error| error.to_string())?;
            let _ = fs::remove_dir_all(&output_dir);

            emit_stage("done");
            eprintln!(
                "live-poly-trans tauri: reprocess-done id={id} segments={}",
                lines.len()
            );
            Ok(lines.len())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Copies an external audio file (phone memo, voice recorder) into a new
    /// recording so it can be played, trimmed, and later re-processed.
    #[tauri::command]
    pub async fn import_audio_file(
        app: AppHandle,
        path: String,
    ) -> Result<CreatedRecording, String> {
        tauri::async_runtime::spawn_blocking(move || {
            let source = PathBuf::from(&path);
            let extension = source
                .extension()
                .and_then(|ext| ext.to_str())
                .map(str::to_lowercase)
                .unwrap_or_default();
            if !matches!(extension.as_str(), "m4a" | "wav" | "mp3" | "aac") {
                return Err(format!(
                    "unsupported audio format: .{extension} (m4a / wav / mp3 / aac のみ読み込めます)"
                ));
            }
            if !source.exists() {
                return Err(format!("file not found: {path}"));
            }

            let id = format!("rec-{}", readable_timestamp());
            let dir = recording_dir_for(&app, &id)?;
            fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            fs::copy(&source, dir.join(format!("external.{extension}")))
                .map_err(|error| error.to_string())?;

            let now = chrono::Local::now().to_rfc3339();
            write_recording_meta(
                &dir,
                &RecordingMeta {
                    id: id.clone(),
                    started_at: now.clone(),
                    ended_at: Some(now),
                    source: Some("external".to_string()),
                    trims: None,
                    speakers: None,
                    actions: None,
                },
            )?;

            Ok(CreatedRecording {
                id,
                dir: dir.display().to_string(),
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Writes the kept range into a new `<stem>-trimmed.m4a` (original file
    /// untouched) and remembers the range in meta so the UI can shift the
    /// transcript for the trimmed file.
    #[tauri::command]
    pub async fn trim_recording(
        app: AppHandle,
        id: String,
        file_name: String,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<String, String> {
        if file_name.contains('/') || file_name.contains("..") {
            return Err(format!("invalid recording file name: {file_name}"));
        }
        if end_ms <= start_ms || start_ms < 0 {
            return Err(format!("invalid trim range: {start_ms}ms - {end_ms}ms"));
        }

        tauri::async_runtime::spawn_blocking(move || {
            let dir = recording_dir_for(&app, &id)?;
            let input = dir.join(&file_name);
            if !input.exists() {
                return Err(format!("recording file not found: {file_name}"));
            }

            let stem = file_name
                .rsplit_once('.')
                .map(|(stem, _)| stem)
                .unwrap_or(&file_name);
            let output = unique_destination(&dir, &format!("{stem}-trimmed"), "m4a");

            let helper_path = resolve_helper_path()?;
            let result = Command::new(helper_path)
                .arg("--trim")
                .arg("--input")
                .arg(&input)
                .arg("--output")
                .arg(&output)
                .arg("--start-ms")
                .arg(start_ms.to_string())
                .arg("--end-ms")
                .arg(end_ms.to_string())
                .output()
                .map_err(|error| error.to_string())?;

            if !result.status.success() {
                return Err(String::from_utf8_lossy(&result.stderr).trim().to_owned());
            }

            let output_name = output
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();

            if let Some(mut meta) = read_recording_meta(&dir) {
                meta.trims.get_or_insert_with(HashMap::new).insert(
                    output_name.clone(),
                    TrimRange {
                        source_file: file_name.clone(),
                        start_ms,
                        end_ms,
                    },
                );
                write_recording_meta(&dir, &meta)?;
            }

            Ok(output_name)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Saves the recording's speaker names/colors into meta.json. The
    /// transcript jsonl stays untouched — names resolve at display time.
    #[tauri::command]
    pub async fn update_recording_speakers(
        app: AppHandle,
        id: String,
        speakers: HashMap<String, SpeakerMeta>,
    ) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || {
            let dir = recording_dir_for(&app, &id)?;
            update_recording_speakers_in_dir(&dir, speakers)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Saves AI action items into meta.json. The frontend owns extraction and
    /// checkbox state; Rust only persists the current snapshot.
    #[tauri::command]
    pub async fn update_recording_actions(
        app: AppHandle,
        id: String,
        actions: Vec<RecordingActionItem>,
    ) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || {
            let dir = recording_dir_for(&app, &id)?;
            update_recording_actions_in_dir(&dir, actions)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn list_recordings(app: AppHandle) -> Result<Vec<RecordingSummary>, String> {
        tauri::async_runtime::spawn_blocking(move || list_recordings_blocking(&app))
            .await
            .map_err(|error| error.to_string())?
    }

    fn list_recordings_blocking(app: &AppHandle) -> Result<Vec<RecordingSummary>, String> {
        let root = recordings_dir(app)?;
        let Ok(entries) = fs::read_dir(&root) else {
            return Ok(Vec::new());
        };

        let mut recordings = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }

            let Some(meta) = read_recording_meta(&dir) else {
                continue;
            };

            let mut files = Vec::new();
            if let Ok(dir_entries) = fs::read_dir(&dir) {
                for file_entry in dir_entries.flatten() {
                    let path = file_entry.path();
                    if path.extension().and_then(|ext| ext.to_str()) != Some("m4a") {
                        continue;
                    }

                    let name = file_entry.file_name().to_string_lossy().into_owned();
                    let size_bytes = file_entry.metadata().map(|meta| meta.len()).unwrap_or(0);
                    files.push(RecordingFileInfo {
                        stream: recording_stream_name(&name),
                        name,
                        path: path.display().to_string(),
                        size_bytes,
                    });
                }
            }

            files.sort_by(|left, right| left.name.cmp(&right.name));
            recordings.push(RecordingSummary {
                id: meta.id,
                started_at: meta.started_at,
                ended_at: meta.ended_at,
                source: meta.source,
                trims: meta.trims,
                speakers: meta.speakers,
                actions: meta.actions,
                files,
            });
        }

        recordings.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        Ok(recordings)
    }

    #[tauri::command]
    pub async fn read_recording_transcript(
        app: AppHandle,
        id: String,
    ) -> Result<Vec<Value>, String> {
        // An 8-hour session's jsonl runs to tens of MB; parsing it on the
        // main thread freezes the whole window.
        tauri::async_runtime::spawn_blocking(move || read_recording_transcript_blocking(&app, &id))
            .await
            .map_err(|error| error.to_string())?
    }

    fn read_recording_transcript_blocking(app: &AppHandle, id: &str) -> Result<Vec<Value>, String> {
        let dir = recording_dir_for(app, id)?;
        Ok(read_recording_transcript_events(&dir))
    }

    #[tauri::command]
    pub async fn search_recordings(
        app: AppHandle,
        query: String,
    ) -> Result<Vec<RecordingSearchHit>, String> {
        tauri::async_runtime::spawn_blocking(move || {
            let root = recordings_dir(&app)?;
            search_recordings_in_root(&root, &query)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn recording_waveform(
        app: AppHandle,
        id: String,
        file_name: String,
    ) -> Result<Value, String> {
        if file_name.contains('/') || file_name.contains("..") {
            return Err(format!("invalid recording file name: {file_name}"));
        }

        let path = recording_dir_for(&app, &id)?.join(&file_name);
        if !path.exists() {
            return Err(format!("recording file not found: {file_name}"));
        }

        tauri::async_runtime::spawn_blocking(move || {
            let helper_path = resolve_helper_path()?;
            let output = Command::new(helper_path)
                .arg("--waveform")
                .arg(&path)
                .output()
                .map_err(|error| error.to_string())?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
            }

            serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn export_recording(
        app: AppHandle,
        id: String,
        variant: String,
    ) -> Result<String, String> {
        let dir = recording_dir_for(&app, &id)?;
        let downloads = app
            .path()
            .download_dir()
            .map_err(|error| error.to_string())?;

        tauri::async_runtime::spawn_blocking(move || {
            let source = match variant.as_str() {
                "mic" | "speaker" => {
                    let path = dir.join(format!("{variant}.m4a"));
                    if !path.exists() {
                        return Err(format!("no {variant} recording in {id}"));
                    }
                    path
                }
                "mixed" => {
                    let mic = dir.join("mic.m4a");
                    let speaker = dir.join("speaker.m4a");
                    if !mic.exists() || !speaker.exists() {
                        return Err(
                            "mixed export needs both mic and speaker recordings".to_string()
                        );
                    }

                    let mixed = dir.join("mixed.m4a");
                    let is_stale = match (fs::metadata(&mixed), fs::metadata(&mic)) {
                        (Ok(mixed_meta), Ok(mic_meta)) => {
                            matches!(
                                (mixed_meta.modified(), mic_meta.modified()),
                                (Ok(mixed_time), Ok(mic_time)) if mixed_time < mic_time
                            )
                        }
                        _ => true,
                    };

                    if is_stale {
                        let helper_path = resolve_helper_path()?;
                        let output = Command::new(helper_path)
                            .arg("--mix")
                            .arg("--input")
                            .arg(&mic)
                            .arg("--input")
                            .arg(&speaker)
                            .arg("--output")
                            .arg(&mixed)
                            .output()
                            .map_err(|error| error.to_string())?;

                        if !output.status.success() {
                            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
                        }
                    }
                    mixed
                }
                other => return Err(format!("unknown export variant: {other}")),
            };

            let destination =
                unique_destination(&downloads, &format!("LivePolyTrans-{id}-{variant}"), "m4a");
            fs::copy(&source, &destination).map_err(|error| error.to_string())?;
            Ok(destination.display().to_string())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn save_text_file(path: String, contents: String) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || write_text_file(&path, &contents))
            .await
            .map_err(|error| error.to_string())?
    }

    #[tauri::command]
    pub async fn validate_export_directory(path: String) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || {
            validate_export_directory_path(Path::new(&path))
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(HelperSession::default())
        .invoke_handler(tauri::generate_handler![
            commands::detect_languages,
            commands::install_language,
            commands::uninstall_language,
            commands::start_stream_session,
            commands::stop_stream_session,
            commands::list_speech_models,
            commands::stop_all_sessions,
            commands::save_transcript,
            commands::ai_generate_summary,
            commands::ai_suggest_questions,
            commands::ai_extract_actions,
            commands::ai_ask,
            commands::toggle_overlay,
            commands::begin_overlay_adjustment,
            commands::start_overlay_drag,
            commands::finish_overlay_adjustment,
            commands::overlay_adjustment_enabled,
            commands::configure_global_shortcuts,
            commands::create_recording,
            commands::finalize_recording,
            commands::start_recording_session,
            commands::stop_recording_session,
            commands::recordings_directory,
            commands::reveal_recordings_directory,
            commands::import_audio_file,
            commands::trim_recording,
            commands::update_recording_speakers,
            commands::update_recording_actions,
            commands::whisperx_status,
            commands::ensure_uv,
            commands::permission_status,
            commands::request_permission,
            commands::open_privacy_settings,
            commands::reprocess_recording,
            commands::delete_recording,
            commands::list_recordings,
            commands::read_recording_transcript,
            commands::search_recordings,
            commands::recording_waveform,
            commands::export_recording,
            commands::save_text_file,
            commands::validate_export_directory
        ])
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray-show-main" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "tray-quit" => app.exit(0),
            id => {
                if let Some(command) = tray_command_for_menu_id(id) {
                    let _ = app.emit("tray-command", command);
                }
            }
        })
        .setup(|app| {
            let _ = app.get_webview_window("main");
            setup_tray(app.handle())?;
            if let Err(error) = setup_global_shortcuts(app.handle()) {
                let message = format!("Could not register global shortcuts: {error}");
                eprintln!("{message}");
                let _ = app.emit("shortcut-error", message);
            }
            Ok(())
        })
        .manage(OverlayAdjustmentState::default())
        .build(tauri::generate_context!())
        .expect("error while running LivePolyTrans")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                shutdown_capture_on_exit(app);
            }
        });
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(
                app,
                "tray-toggle-recording",
                "録音 開始/停止",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app,
                "tray-toggle-pause",
                "一時停止/再開",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "tray-toggle-overlay",
                "字幕オーバーレイ",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(
                app,
                "tray-adjust-overlay",
                "オーバーレイの位置調整",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "tray-show-main",
                "メイン画面を表示",
                true,
                None::<&str>,
            )?,
            &MenuItem::with_id(app, "tray-quit", "終了", true, None::<&str>)?,
        ],
    )?;

    let mut tray = TrayIconBuilder::with_id("live-poly-trans")
        .menu(&menu)
        .tooltip("LivePolyTrans")
        .show_menu_on_left_click(true)
        .icon_as_template(true);
    tray = tray.icon(tray_waveform_icon(false));

    tray.build(app)?;
    Ok(())
}

pub fn tray_waveform_icon(recording: bool) -> Image<'static> {
    Image::new_owned(
        tray_waveform_icon_rgba(recording),
        TRAY_WAVEFORM_ICON_SIZE,
        TRAY_WAVEFORM_ICON_SIZE,
    )
}

pub fn tray_waveform_icon_rgba(recording: bool) -> Vec<u8> {
    let size = TRAY_WAVEFORM_ICON_SIZE;
    let mut rgba = vec![0; (size * size * 4) as usize];
    for (x, y, width, height) in [
        (3, 6, 2, 7),
        (6, 3, 2, 12),
        (9, 5, 2, 9),
        (12, 4, 2, 11),
    ] {
        set_rgba_rect(&mut rgba, size, x, y, width, height, [0, 0, 0, 255]);
    }

    if recording {
        set_rgba_rect(&mut rgba, size, 14, 2, 3, 3, [255, 59, 48, 255]);
    }

    rgba
}

pub fn set_rgba_rect(
    rgba: &mut [u8],
    image_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    for row in y..(y + height) {
        for column in x..(x + width) {
            let start = ((row * image_width + column) * 4) as usize;
            if start + 4 <= rgba.len() {
                rgba[start..start + 4].copy_from_slice(&color);
            }
        }
    }
}

pub fn rgba_pixel(rgba: &[u8], image_width: u32, x: u32, y: u32) -> [u8; 4] {
    let start = ((y * image_width + x) * 4) as usize;
    [
        rgba.get(start).copied().unwrap_or(0),
        rgba.get(start + 1).copied().unwrap_or(0),
        rgba.get(start + 2).copied().unwrap_or(0),
        rgba.get(start + 3).copied().unwrap_or(0),
    ]
}

fn setup_global_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_shortcuts(["CommandOrControl+Alt+R", "CommandOrControl+Alt+L"])?
            .with_handler(|app, shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                if let Some(command) = shortcut_command(shortcut) {
                    let _ = app.emit("tray-command", command);
                }
            })
            .build(),
    )?;
    Ok(())
}

fn configure_global_shortcuts_for_app(
    app: &AppHandle,
    enabled: bool,
    recording_shortcut: &str,
    overlay_shortcut: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    app.global_shortcut().unregister_all()?;
    if !enabled {
        return Ok(());
    }

    let recording = recording_shortcut.trim();
    if !recording.is_empty() {
        app.global_shortcut()
            .on_shortcut(recording, |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    let _ = app.emit("tray-command", TRAY_COMMAND_TOGGLE_RECORDING);
                }
            })?;
    }

    let overlay = overlay_shortcut.trim();
    if !overlay.is_empty() {
        app.global_shortcut()
            .on_shortcut(overlay, |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    let _ = app.emit("tray-command", TRAY_COMMAND_TOGGLE_OVERLAY);
                }
            })?;
    }

    Ok(())
}

/// Quitting mid-recording must still drain the helpers (so the m4a files get
/// their moov atoms) and close the recording metadata; the frontend's stop
/// path never runs on ⌘Q.
fn shutdown_capture_on_exit(app: &AppHandle) {
    let session = app.state::<HelperSession>();

    let streams: Vec<String> = match session.children.lock() {
        Ok(children) => children.keys().cloned().collect(),
        Err(_) => Vec::new(),
    };

    // Streams stop in parallel: sequential graceful stops could take
    // stop_grace per stream and macOS force-kills apps that linger on quit.
    let handles: Vec<_> = streams
        .into_iter()
        .map(|stream| {
            let children = session.children.clone();
            std::thread::spawn(move || {
                let _ = stop_stream_child(&children, &stream);
            })
        })
        .collect();
    for handle in handles {
        let _ = handle.join();
    }

    if let Ok(root) = recordings_dir(app) {
        close_open_recordings(&root, &chrono::Local::now().to_rfc3339());
    }
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
    fn whisper_engine_binary_names_cover_current_distribution_targets() {
        assert_eq!(
            whisper_engine_binary_name_for("aarch64", "macos", ""),
            "lpt-whisper-engine-aarch64-apple-darwin"
        );
        assert_eq!(
            whisper_engine_binary_name_for("x86_64", "macos", ""),
            "lpt-whisper-engine-x86_64-apple-darwin"
        );
        assert_eq!(
            whisper_engine_binary_name_for("x86_64", "windows", "msvc"),
            "lpt-whisper-engine-x86_64-pc-windows-msvc.exe"
        );
        assert_eq!(
            whisper_engine_executable_name_for("windows"),
            "lpt-whisper-engine.exe"
        );
    }

    #[test]
    fn whisper_engine_candidates_include_development_binary_path() {
        let candidates = whisper_engine_sidecar_candidates(
            Path::new("/repo/src-tauri"),
            Path::new("/repo/src-tauri/target/debug/live-poly-trans"),
        );

        assert!(candidates.contains(&PathBuf::from(
            "/repo/src-tauri/target/debug/lpt-whisper-engine"
        )));
        assert!(candidates.contains(&PathBuf::from(
            "/repo/src-tauri/binaries/lpt-whisper-engine"
        )));
    }

    #[test]
    fn whisper_engine_candidates_include_bundled_sidecar_path() {
        let candidates = whisper_engine_sidecar_candidates(
            Path::new("/repo/src-tauri"),
            Path::new("/App/LivePolyTrans.app/Contents/MacOS/live-poly-trans"),
        );

        assert!(candidates.contains(&PathBuf::from(
            "/App/LivePolyTrans.app/Contents/MacOS/../Resources/lpt-whisper-engine"
        )));
        assert!(candidates.contains(&PathBuf::from(
            "/App/LivePolyTrans.app/Contents/MacOS/../Resources/binaries/lpt-whisper-engine"
        )));
    }

    #[test]
    fn whisperx_runner_prefers_uvx_then_pipx_then_direct() {
        let uvx = whisperx_runner_from(|name| {
            (name == "uvx").then(|| "/opt/homebrew/bin/uvx".to_string())
        })
        .unwrap();
        assert_eq!(uvx.program, "/opt/homebrew/bin/uvx");
        assert_eq!(uvx.prefix_args, vec!["whisperx"]);

        let pipx = whisperx_runner_from(|name| {
            (name == "pipx").then(|| "/usr/local/bin/pipx".to_string())
        })
        .unwrap();
        assert_eq!(pipx.prefix_args, vec!["run", "whisperx"]);

        let direct = whisperx_runner_from(|name| {
            (name == "whisperx").then(|| "/usr/local/bin/whisperx".to_string())
        })
        .unwrap();
        assert!(direct.prefix_args.is_empty());

        assert_eq!(whisperx_runner_from(|_| None), None);
    }

    #[test]
    fn uv_download_pins_a_url_and_checksum_per_architecture() {
        let arm = uv_download_for("aarch64").unwrap();
        assert!(arm.url.contains(UV_VERSION));
        assert!(arm.url.ends_with("uv-aarch64-apple-darwin.tar.gz"));
        assert_eq!(arm.archive_dir, "uv-aarch64-apple-darwin");
        assert_eq!(arm.sha256.len(), 64);

        let intel = uv_download_for("x86_64").unwrap();
        assert_eq!(intel.archive_dir, "uv-x86_64-apple-darwin");
        assert_ne!(intel.sha256, arm.sha256);

        assert!(uv_download_for("riscv64").is_none());
    }

    #[test]
    fn program_candidates_prefer_the_app_managed_directory() {
        let managed = PathBuf::from("/Data/tools/uv");
        let candidates = program_candidates("uvx", "/Users/me", Some(&managed));

        assert_eq!(candidates.first().unwrap(), &managed.join("uvx"));
        assert!(candidates.contains(&PathBuf::from("/opt/homebrew/bin/uvx")));
        assert!(candidates.contains(&PathBuf::from("/Users/me/.local/bin/uvx")));

        let without_managed = program_candidates("uvx", "/Users/me", None);
        assert!(!without_managed.iter().any(|path| path.starts_with("/Data")));
    }

    #[test]
    fn sha256_is_read_from_the_shasum_output_column() {
        let digest = "77d2906988e8074fd43f2f329ec452ebbf9b0c257ba1c66451c71de70a6baf42";
        assert_eq!(
            sha256_from_shasum_output(&format!("{digest}  /tmp/uv.tar.gz\n")),
            Some(digest.to_string())
        );
        assert_eq!(sha256_from_shasum_output("shasum: no such file"), None);
        assert_eq!(sha256_from_shasum_output(""), None);
    }

    #[test]
    fn uv_install_downloads_verifies_then_extracts() {
        let download = uv_download_for("aarch64").unwrap();
        let calls = std::cell::RefCell::new(Vec::new());
        let stages = std::cell::RefCell::new(Vec::new());

        let result = install_uv_with(
            &download,
            Path::new("/work"),
            Path::new("/dest"),
            |program, args| {
                calls
                    .borrow_mut()
                    .push(format!("{program} {}", args.join(" ")));
                Ok(if program.ends_with("shasum") {
                    format!("{}  /work/uv.tar.gz", download.sha256)
                } else {
                    String::new()
                })
            },
            |stage| stages.borrow_mut().push(stage.to_string()),
        );

        assert!(result.is_ok());
        let calls = calls.into_inner();
        assert!(calls[0].starts_with("/usr/bin/curl"), "{}", calls[0]);
        assert!(calls[0].contains(&download.url));
        assert!(
            calls[1].starts_with("/usr/bin/shasum -a 256"),
            "{}",
            calls[1]
        );
        assert!(calls[2].starts_with("/usr/bin/tar -xzf"), "{}", calls[2]);
        assert_eq!(
            stages.into_inner(),
            vec!["download", "verify", "extract", "done"]
        );
    }

    #[test]
    fn uv_install_stops_before_extracting_when_the_checksum_differs() {
        let download = uv_download_for("aarch64").unwrap();
        let calls = std::cell::RefCell::new(Vec::new());

        let result = install_uv_with(
            &download,
            Path::new("/work"),
            Path::new("/dest"),
            |program, _| {
                calls.borrow_mut().push(program.to_string());
                Ok(if program.ends_with("shasum") {
                    format!("{}  /work/uv.tar.gz", "0".repeat(64))
                } else {
                    String::new()
                })
            },
            |_| {},
        );

        assert!(result.unwrap_err().contains("チェックサム"));
        assert!(!calls.into_inner().iter().any(|call| call.ends_with("tar")));
    }

    #[test]
    fn privacy_settings_urls_target_the_matching_pane() {
        assert_eq!(
            privacy_settings_url("microphone"),
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        );
        assert_eq!(
            privacy_settings_url("screen-recording"),
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        );
        assert_eq!(privacy_settings_url("camera"), None);
    }

    #[test]
    fn whisperx_stages_map_from_stderr_markers() {
        assert_eq!(
            whisperx_stage_for_line(">>Performing transcription..."),
            Some("transcribe")
        );
        assert_eq!(
            whisperx_stage_for_line(">>Performing alignment..."),
            Some("align")
        );
        assert_eq!(
            whisperx_stage_for_line(">>Performing diarization..."),
            Some("diarize")
        );
        assert_eq!(whisperx_stage_for_line("Loading model..."), None);
    }

    #[test]
    fn whisperx_reprocess_args_skip_unused_alignment_model() {
        let args = whisperx_reprocess_args(
            Path::new("/recordings/mixed.m4a"),
            Path::new("/recordings/whisperx-tmp"),
            None,
        );

        assert!(args.contains(&"--no_align".to_string()));
        assert!(args
            .windows(2)
            .any(|window| window[0] == "--model" && window[1] == "large-v3-turbo"));
        assert!(!args.contains(&"--diarize".to_string()));
        assert!(!args.contains(&"--hf_token".to_string()));
    }

    #[test]
    fn whisperx_reprocess_args_keep_diarization_when_token_is_present() {
        let args = whisperx_reprocess_args(
            Path::new("/recordings/mixed.m4a"),
            Path::new("/recordings/whisperx-tmp"),
            Some("hf_xxx"),
        );

        assert!(args.contains(&"--no_align".to_string()));
        assert!(args.contains(&"--diarize".to_string()));
        assert!(args
            .windows(2)
            .any(|window| window[0] == "--hf_token" && window[1] == "hf_xxx"));
    }

    #[test]
    fn whisperx_output_converts_to_jsonl_lines() {
        let output = serde_json::json!({
            "language": "ja",
            "segments": [
                { "start": 0.5, "end": 2.25, "text": " こんにちは ", "speaker": "SPEAKER_00" },
                { "start": 2.5, "end": 4.0, "text": "はい" },
                { "start": 4.0, "end": 4.5, "text": "   " }
            ]
        });

        let lines = whisperx_jsonl_lines(&output);
        assert_eq!(lines.len(), 2);

        let first: Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(first["type"], "whisperx");
        assert_eq!(first["startMs"], 500);
        assert_eq!(first["endMs"], 2250);
        assert_eq!(first["text"], "こんにちは");
        assert_eq!(first["speaker"], "SPEAKER_00");
        assert_eq!(first["lang"], "ja");

        let second: Value = serde_json::from_str(&lines[1]).unwrap();
        assert!(second.get("speaker").is_none());
    }

    #[test]
    fn recording_control_lines_match_the_helper_protocol() {
        assert_eq!(
            start_recording_control_line("/tmp/rec-1", true),
            r#"{"audio":true,"cmd":"start-recording","dir":"/tmp/rec-1"}"#
        );
        assert_eq!(
            start_recording_control_line("/tmp/rec-1", false),
            r#"{"audio":false,"cmd":"start-recording","dir":"/tmp/rec-1"}"#
        );
        assert_eq!(stop_recording_control_line(), r#"{"cmd":"stop-recording"}"#);
    }

    #[test]
    fn start_recording_control_line_escapes_special_characters() {
        let line = start_recording_control_line(r#"/tmp/we"ird dir"#, true);
        let value: Value = serde_json::from_str(&line).expect("control line is valid JSON");
        assert_eq!(value["dir"], r#"/tmp/we"ird dir"#);
    }

    #[test]
    fn control_ready_is_detected_only_for_its_status_event() {
        assert!(is_control_ready_event(&serde_json::json!({
            "type": "status", "stream": "mic", "state": "control-ready"
        })));
        assert!(!is_control_ready_event(&serde_json::json!({
            "type": "status", "stream": "mic", "state": "language-ready"
        })));
        assert!(!is_control_ready_event(&serde_json::json!({
            "type": "transcript", "state": "control-ready"
        })));
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
    fn tauri_transcript_logs_only_final_events() {
        let volatile = serde_json::json!({
            "type": "transcript",
            "isFinal": false,
            "text": "partial"
        });
        let final_event = serde_json::json!({
            "type": "transcript",
            "isFinal": true,
            "text": "complete"
        });

        assert!(!should_log_transcript_event(&volatile));
        assert!(should_log_transcript_event(&final_event));
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
            "timestamp": "2026-06-20T00:00:00Z",
            "confidence": 0.75,
            "spans": [{ "text": "hello", "confidence": 0.75, "startMs": 100, "endMs": 600 }]
        }))
        .unwrap();

        assert_eq!(message.speaker_id.as_deref(), Some("system-audio"));
        assert_eq!(message.speaker_label.as_deref(), Some("Speaker"));
        assert_eq!(message.confidence, Some(0.75));
        assert_eq!(message.spans.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn builds_ai_context_from_selected_saved_messages() {
        let messages = vec![SavedTranscriptMessage {
            role: "speaker".to_string(),
            speaker_id: Some("system-audio".to_string()),
            speaker_label: Some("Speaker B".to_string()),
            language: "ja-JP".to_string(),
            text: "次のリリースは金曜日です".to_string(),
            translation: Some("The next release is Friday.".to_string()),
            timestamp: "2026-06-23T10:00:00Z".to_string(),
            confidence: Some(0.88),
            spans: None,
        }];

        assert_eq!(
            build_ai_context_from_saved_messages(&messages),
            "[2026-06-23T10:00:00Z] Speaker B / ja-JP: 次のリリースは金曜日です\n  => The next release is Friday."
        );
    }

    #[test]
    fn builds_stream_helper_args_with_recording_files() {
        let args = build_stream_helper_args(
            "mic",
            "en-US",
            "ja-JP",
            &["en-US".to_string(), "ja-JP".to_string()],
            "/tmp/segments/mic",
            Some("/tmp/rec/mic.m4a"),
            Some("/tmp/rec/mic.jsonl"),
            None,
        );

        assert_eq!(
            args,
            vec![
                "--stream",
                "mic",
                "--source-language",
                "en-US",
                "--target-language",
                "ja-JP",
                "--segment-directory",
                "/tmp/segments/mic",
                "--language",
                "en-US",
                "--language",
                "ja-JP",
                "--record-file",
                "/tmp/rec/mic.m4a",
                "--transcript-file",
                "/tmp/rec/mic.jsonl",
            ]
        );
    }

    #[test]
    fn builds_stream_helper_args_without_recording() {
        let args = build_stream_helper_args(
            "speaker",
            "en-US",
            "ja-JP",
            &[],
            "/tmp/segments/speaker",
            None,
            None,
            None,
        );

        assert!(!args.contains(&"--record-file".to_string()));
        assert!(!args.contains(&"--transcript-file".to_string()));
    }

    #[test]
    fn lists_ggml_models_with_sizes() {
        let dir = std::env::temp_dir().join(format!("lpt-models-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("ggml-large-v3-turbo.bin"), b"12345").unwrap();
        fs::write(dir.join("ggml-base.en.bin"), b"123").unwrap();
        fs::write(dir.join("not-a-model.txt"), b"x").unwrap();
        fs::create_dir_all(dir.join("models")).unwrap();

        let models = scan_speech_models(&dir);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].file_name, "ggml-base.en.bin");
        assert_eq!(models[0].size_bytes, 3);
        assert_eq!(models[1].file_name, "ggml-large-v3-turbo.bin");
        assert_eq!(models[1].size_bytes, 5);
    }

    #[test]
    fn scans_missing_models_directory_as_empty() {
        assert!(scan_speech_models(Path::new("/nonexistent/lpt-models")).is_empty());
    }

    #[test]
    fn whisper_cli_candidates_include_homebrew_paths() {
        let candidates = whisper_cli_candidates();

        assert!(candidates.contains(&PathBuf::from("/opt/homebrew/bin/whisper-cli")));
        assert!(candidates.contains(&PathBuf::from("/usr/local/bin/whisper-cli")));
    }

    #[test]
    fn stop_grace_for_engine_extends_whisper_shutdown() {
        assert_eq!(
            stop_grace_for_engine(Some("whisper")),
            Duration::from_secs(15)
        );
        // Must stay longer than the Swift helper's 5s shutdown safety net
        // (MicrophoneTranscriber.installShutdownHandlers), which finalizes the
        // recording file before exiting. Killing earlier truncates the m4a.
        assert_eq!(
            stop_grace_for_engine(Some("builtin")),
            Duration::from_secs(7)
        );
        assert_eq!(stop_grace_for_engine(None), Duration::from_secs(7));
    }

    #[test]
    fn close_open_recordings_finalizes_only_recordings_without_end() {
        let root =
            std::env::temp_dir().join(format!("lpt-close-open-recordings-{}", std::process::id()));
        let open_dir = root.join("rec-open");
        let closed_dir = root.join("rec-closed");
        fs::create_dir_all(&open_dir).unwrap();
        fs::create_dir_all(&closed_dir).unwrap();

        write_recording_meta(
            &open_dir,
            &RecordingMeta {
                id: "rec-open".to_string(),
                started_at: "2026-07-31T10:00:00+09:00".to_string(),
                ended_at: None,
                source: None,
                trims: None,
                speakers: None,
                actions: None,
            },
        )
        .unwrap();
        write_recording_meta(
            &closed_dir,
            &RecordingMeta {
                id: "rec-closed".to_string(),
                started_at: "2026-07-31T09:00:00+09:00".to_string(),
                ended_at: Some("2026-07-31T09:30:00+09:00".to_string()),
                source: None,
                trims: None,
                speakers: None,
                actions: None,
            },
        )
        .unwrap();

        close_open_recordings(&root, "2026-07-31T11:00:00+09:00");

        let open_meta = read_recording_meta(&open_dir).unwrap();
        let closed_meta = read_recording_meta(&closed_dir).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(
            open_meta.ended_at.as_deref(),
            Some("2026-07-31T11:00:00+09:00")
        );
        assert_eq!(
            closed_meta.ended_at.as_deref(),
            Some("2026-07-31T09:30:00+09:00")
        );
    }

    #[test]
    fn recording_meta_round_trips_speakers() {
        let root = std::env::temp_dir().join(format!("lpt-meta-speakers-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();

        let mut speakers = HashMap::new();
        speakers.insert(
            "speaker-0".to_string(),
            SpeakerMeta {
                name: "田中さん".to_string(),
                color: Some("#123456".to_string()),
            },
        );
        speakers.insert(
            "mic".to_string(),
            SpeakerMeta {
                name: "自分".to_string(),
                color: None,
            },
        );

        write_recording_meta(
            &root,
            &RecordingMeta {
                id: "rec-1".to_string(),
                started_at: "2026-08-05T10:00:00+09:00".to_string(),
                ended_at: None,
                source: None,
                trims: None,
                speakers: Some(speakers),
                actions: None,
            },
        )
        .unwrap();

        let meta = read_recording_meta(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let speakers = meta.speakers.unwrap();
        assert_eq!(speakers["speaker-0"].name, "田中さん");
        assert_eq!(speakers["speaker-0"].color.as_deref(), Some("#123456"));
        assert_eq!(speakers["mic"].name, "自分");
        assert_eq!(speakers["mic"].color, None);
    }

    #[test]
    fn recording_meta_without_speakers_still_deserializes() {
        let legacy = r#"{"id":"rec-1","startedAt":"2026-08-05T10:00:00+09:00"}"#;
        let meta: RecordingMeta = serde_json::from_str(legacy).unwrap();
        assert!(meta.speakers.is_none());
        assert!(meta.actions.is_none());

        // A meta without optional keys must not gain them on rewrite.
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("speakers"));
        assert!(!json.contains("actions"));
    }

    #[test]
    fn update_recording_speakers_in_dir_saves_the_map_into_meta() {
        let root = std::env::temp_dir().join(format!("lpt-update-speakers-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        write_recording_meta(
            &root,
            &RecordingMeta {
                id: "rec-1".to_string(),
                started_at: "2026-08-05T10:00:00+09:00".to_string(),
                ended_at: Some("2026-08-05T10:30:00+09:00".to_string()),
                source: None,
                trims: None,
                speakers: None,
                actions: None,
            },
        )
        .unwrap();

        let mut speakers = HashMap::new();
        speakers.insert(
            "speaker-1".to_string(),
            SpeakerMeta {
                name: "佐藤さん".to_string(),
                color: None,
            },
        );

        let result = update_recording_speakers_in_dir(&root, speakers);
        let meta = read_recording_meta(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(result, Ok(()));
        // The rest of the meta survives the rewrite.
        assert_eq!(meta.ended_at.as_deref(), Some("2026-08-05T10:30:00+09:00"));
        assert_eq!(meta.speakers.unwrap()["speaker-1"].name, "佐藤さん");
    }

    #[test]
    fn update_recording_speakers_in_dir_clears_meta_with_an_empty_map() {
        let root =
            std::env::temp_dir().join(format!("lpt-update-speakers-clear-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();

        let mut speakers = HashMap::new();
        speakers.insert(
            "speaker-0".to_string(),
            SpeakerMeta {
                name: "田中さん".to_string(),
                color: None,
            },
        );
        write_recording_meta(
            &root,
            &RecordingMeta {
                id: "rec-1".to_string(),
                started_at: "2026-08-05T10:00:00+09:00".to_string(),
                ended_at: None,
                source: None,
                trims: None,
                speakers: Some(speakers),
                actions: None,
            },
        )
        .unwrap();

        let result = update_recording_speakers_in_dir(&root, HashMap::new());
        let meta = read_recording_meta(&root).unwrap();
        let raw = fs::read_to_string(root.join("meta.json")).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(result, Ok(()));
        assert!(meta.speakers.is_none());
        assert!(!raw.contains("speakers"));
    }

    #[test]
    fn update_recording_speakers_in_dir_fails_without_meta() {
        let root = std::env::temp_dir().join(format!(
            "lpt-update-speakers-missing-{}",
            std::process::id()
        ));

        let result = update_recording_speakers_in_dir(&root, HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn update_recording_actions_in_dir_saves_items_into_meta() {
        let root = std::env::temp_dir().join(format!("lpt-update-actions-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        write_recording_meta(
            &root,
            &RecordingMeta {
                id: "rec-1".to_string(),
                started_at: "2026-08-05T10:00:00+09:00".to_string(),
                ended_at: Some("2026-08-05T10:30:00+09:00".to_string()),
                source: None,
                trims: None,
                speakers: None,
                actions: None,
            },
        )
        .unwrap();

        let result = update_recording_actions_in_dir(
            &root,
            vec![RecordingActionItem {
                id: "a-1".to_string(),
                text: "告知文を書く".to_string(),
                assignee: Some("自分".to_string()),
                timestamp_ms: Some(63_000),
                source_index: Some(2),
                done: true,
            }],
        );
        let meta = read_recording_meta(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(result, Ok(()));
        assert_eq!(meta.ended_at.as_deref(), Some("2026-08-05T10:30:00+09:00"));
        let actions = meta.actions.unwrap();
        assert_eq!(actions[0].text, "告知文を書く");
        assert!(actions[0].done);
    }

    #[test]
    fn update_recording_actions_in_dir_clears_meta_with_an_empty_list() {
        let root =
            std::env::temp_dir().join(format!("lpt-update-actions-clear-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        write_recording_meta(
            &root,
            &RecordingMeta {
                id: "rec-1".to_string(),
                started_at: "2026-08-05T10:00:00+09:00".to_string(),
                ended_at: None,
                source: None,
                trims: None,
                speakers: None,
                actions: Some(vec![RecordingActionItem {
                    id: "a-1".to_string(),
                    text: "告知文を書く".to_string(),
                    assignee: None,
                    timestamp_ms: None,
                    source_index: None,
                    done: false,
                }]),
            },
        )
        .unwrap();

        let result = update_recording_actions_in_dir(&root, Vec::new());
        let meta = read_recording_meta(&root).unwrap();
        let raw = fs::read_to_string(root.join("meta.json")).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(result, Ok(()));
        assert!(meta.actions.is_none());
        assert!(!raw.contains("actions"));
    }

    #[test]
    fn delete_recording_dir_removes_the_directory_with_contents() {
        let root =
            std::env::temp_dir().join(format!("lpt-delete-recording-{}", std::process::id()));
        let dir = root.join("rec-1");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mic.m4a"), b"audio").unwrap();

        let result = delete_recording_dir(&root, "rec-1");
        let still_exists = dir.exists();
        let _ = fs::remove_dir_all(&root);

        assert_eq!(result, Ok(()));
        assert!(!still_exists);
    }

    #[test]
    fn delete_recording_dir_tolerates_missing_directories() {
        let root = std::env::temp_dir().join(format!(
            "lpt-delete-recording-missing-{}",
            std::process::id()
        ));

        assert_eq!(delete_recording_dir(&root, "rec-none"), Ok(()));
    }

    #[test]
    fn delete_recording_dir_rejects_path_traversal() {
        let root = std::env::temp_dir();

        assert!(delete_recording_dir(&root, "../evil").is_err());
        assert!(delete_recording_dir(&root, "a/b").is_err());
    }

    #[test]
    fn write_text_file_writes_contents_to_the_given_path() {
        let dir = std::env::temp_dir().join(format!("lpt-save-text-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("transcript.srt");

        let result = write_text_file(path.to_str().unwrap(), "1\nこんにちは\n");
        let written = fs::read_to_string(&path);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(result, Ok(()));
        assert_eq!(written.unwrap(), "1\nこんにちは\n");
    }

    #[test]
    fn write_text_file_rejects_an_empty_path() {
        assert!(write_text_file("  ", "text").is_err());
    }

    #[test]
    fn validate_export_directory_accepts_existing_writable_directory() {
        let dir = std::env::temp_dir().join(format!("lpt-export-dir-ok-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();

        let result = validate_export_directory_path(&dir);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn validate_export_directory_rejects_empty_and_non_directories() {
        let dir = std::env::temp_dir().join(format!("lpt-export-dir-file-{}", std::process::id()));
        fs::write(&dir, "not a dir").unwrap();

        assert!(validate_export_directory_path(Path::new("")).is_err());
        assert!(validate_export_directory_path(&dir).is_err());

        let _ = fs::remove_file(&dir);
    }

    #[test]
    fn search_transcript_events_matches_case_insensitively_with_snippets() {
        let events = vec![serde_json::json!({
            "type": "transcript",
            "isFinal": true,
            "segmentId": "1200-800",
            "text": "We discussed Claude Code adoption today."
        })];

        let hits = search_transcript_events("rec-1", "claude", &events);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].recording_id, "rec-1");
        assert_eq!(hits[0].entry_index, 0);
        assert_eq!(hits[0].timestamp_ms, 1200);
        assert!(hits[0].snippet.contains("Claude Code"));
    }

    #[test]
    fn search_transcript_events_ignores_blank_queries_and_non_final_transcripts() {
        let events = vec![
            serde_json::json!({
                "type": "transcript",
                "isFinal": false,
                "segmentId": "0-100",
                "text": "Claude"
            }),
            serde_json::json!({
                "type": "translation",
                "segmentId": "0-100",
                "trans": "Claude"
            }),
        ];

        assert!(search_transcript_events("rec-1", " ", &events).is_empty());
        assert!(search_transcript_events("rec-1", "claude", &events).is_empty());
    }

    #[test]
    fn search_recordings_in_root_scans_100_recordings_with_interactive_latency() {
        let root =
            std::env::temp_dir().join(format!("lpt-search-100-recordings-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();

        for recording_index in 0..100 {
            let id = format!("rec-{recording_index:03}");
            let dir = root.join(&id);
            fs::create_dir_all(&dir).unwrap();
            write_recording_meta(
                &dir,
                &RecordingMeta {
                    id: id.clone(),
                    started_at: "2026-08-05T10:00:00+09:00".to_string(),
                    ended_at: Some("2026-08-05T10:30:00+09:00".to_string()),
                    source: None,
                    trims: None,
                    speakers: None,
                    actions: None,
                },
            )
            .unwrap();

            let mut lines = String::new();
            for entry_index in 0..80 {
                let text = if recording_index == 42 && entry_index == 17 {
                    "The team discussed Claude Code rollout.".to_string()
                } else {
                    format!("Routine meeting line {recording_index}-{entry_index}")
                };
                lines.push_str(
                    &serde_json::json!({
                        "type": "transcript",
                        "isFinal": true,
                        "segmentId": format!("{}-500", entry_index * 1000),
                        "text": text
                    })
                    .to_string(),
                );
                lines.push('\n');
            }
            fs::write(dir.join("transcript.jsonl"), lines).unwrap();
        }

        let started = Instant::now();
        let hits = search_recordings_in_root(&root, "claude").unwrap();
        let elapsed = started.elapsed();
        let _ = fs::remove_dir_all(&root);

        eprintln!(
            "search_recordings_100_recordings_elapsed_ms={}",
            elapsed.as_millis()
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].recording_id, "rec-042");
        assert!(
            elapsed < Duration::from_millis(250),
            "100-recording search took {elapsed:?}"
        );
    }

    #[test]
    fn stop_each_stream_attempts_every_stream_before_returning_errors() {
        let streams = vec!["mic".to_string(), "speaker".to_string()];
        let mut attempted = Vec::new();

        let result = stop_each_stream(&streams, |stream| {
            attempted.push(stream.to_string());
            Err(format!("{stream} failed"))
        });

        assert_eq!(attempted, streams);
        assert_eq!(
            result,
            Err("mic: mic failed\nspeaker: speaker failed".to_string())
        );
    }

    #[test]
    fn builds_stream_helper_args_with_whisper_engine() {
        let args = build_stream_helper_args(
            "mic",
            "ja-JP",
            "en-US",
            &[],
            "/tmp/segments/mic",
            None,
            None,
            Some(&WhisperEngineConfig {
                model_path: "/models/ggml-large-v3-turbo.bin",
                cli_path: "/opt/homebrew/bin/whisper-cli",
                engine_path: "/app/lpt-whisper-engine",
            }),
        );

        let tail: Vec<&str> = args
            .iter()
            .rev()
            .take(8)
            .rev()
            .map(String::as_str)
            .collect();
        assert_eq!(
            tail,
            vec![
                "--transcription-engine",
                "whisper",
                "--whisper-model",
                "/models/ggml-large-v3-turbo.bin",
                "--whisper-cli",
                "/opt/homebrew/bin/whisper-cli",
                "--whisper-engine",
                "/app/lpt-whisper-engine",
            ]
        );
    }

    #[test]
    fn omits_engine_flags_for_builtin_engine() {
        let args = build_stream_helper_args(
            "mic",
            "en-US",
            "ja-JP",
            &["en-US".to_string()],
            "/tmp/segments/mic",
            None,
            None,
            None,
        );

        // Builtin sessions must produce byte-identical args to the
        // pre-whisper implementation: zero regression surface.
        assert_eq!(
            args,
            vec![
                "--stream",
                "mic",
                "--source-language",
                "en-US",
                "--target-language",
                "ja-JP",
                "--segment-directory",
                "/tmp/segments/mic",
                "--language",
                "en-US",
            ]
        );
    }

    #[test]
    fn classifies_recording_stream_from_file_name() {
        assert_eq!(recording_stream_name("mic.m4a"), "mic");
        assert_eq!(recording_stream_name("mic-2.m4a"), "mic");
        assert_eq!(recording_stream_name("speaker.m4a"), "speaker");
    }

    #[test]
    fn ai_request_payload_serializes_camel_case_optionals() {
        let request = AiServerRequestPayload {
            id: "1".to_string(),
            command: "summary",
            question: None,
            language: Some("ja-JP"),
            previous_summary: Some("前回の要約"),
            history: None,
            transcript: "本文",
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["previousSummary"], "前回の要約");
        assert_eq!(json["language"], "ja-JP");
        assert!(json.get("question").is_none());
        assert!(json.get("history").is_none());
    }

    #[test]
    fn builds_ai_action_context_with_source_index_and_timestamp_ms() {
        let messages = vec![SavedTranscriptMessage {
            role: "speaker".to_string(),
            speaker_id: Some("self".to_string()),
            speaker_label: Some("Speaker A".to_string()),
            language: "ja-JP".to_string(),
            text: "金曜までに告知文を書きます".to_string(),
            translation: None,
            timestamp: "2026-08-05T10:00:00Z".to_string(),
            confidence: None,
            spans: Some(vec![SavedTranscriptSpan {
                text: "金曜までに告知文を書きます".to_string(),
                confidence: None,
                start_ms: Some(42_000),
                end_ms: Some(45_000),
            }]),
        }];

        let context = build_ai_action_context_from_saved_messages(&messages);

        assert!(context.contains("sourceIndex=0"));
        assert!(context.contains("timestampMs=42000"));
        assert!(context.contains("金曜までに告知文を書きます"));
    }

    #[test]
    fn readable_timestamp_is_filename_safe() {
        let timestamp = readable_timestamp();
        assert_eq!(timestamp.len(), "20260704-120000".len());
        assert!(!timestamp.contains(':'));
        assert!(!timestamp.contains('/'));
    }

    #[test]
    fn overlay_window_state_round_trips_to_json() {
        let root = std::env::temp_dir().join(format!("lpt-overlay-state-{}", std::process::id()));
        let path = root.join("overlay-window.json");
        let state = OverlayWindowState {
            x: 120,
            y: 80,
            width: 980,
            height: 180,
        };

        write_overlay_window_state(&path, &state).unwrap();
        let restored = read_overlay_window_state(&path);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(restored, Some(state));
    }

    #[test]
    fn overlay_window_state_ignores_corrupt_json() {
        let root =
            std::env::temp_dir().join(format!("lpt-overlay-state-corrupt-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("overlay-window.json");
        fs::write(&path, "{broken").unwrap();

        let restored = read_overlay_window_state(&path);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(restored, None);
    }

    #[test]
    fn tray_menu_ids_map_to_frontend_commands() {
        assert_eq!(
            tray_command_for_menu_id("tray-toggle-recording"),
            Some(TRAY_COMMAND_TOGGLE_RECORDING)
        );
        assert_eq!(
            tray_command_for_menu_id("tray-toggle-pause"),
            Some(TRAY_COMMAND_TOGGLE_PAUSE)
        );
        assert_eq!(
            tray_command_for_menu_id("tray-toggle-overlay"),
            Some(TRAY_COMMAND_TOGGLE_OVERLAY)
        );
        assert_eq!(
            tray_command_for_menu_id("tray-adjust-overlay"),
            Some(TRAY_COMMAND_ADJUST_OVERLAY)
        );
        assert_eq!(tray_command_for_menu_id("tray-quit"), None);
    }

    #[test]
    fn tray_waveform_icon_draws_template_bars() {
        let rgba = tray_waveform_icon_rgba(false);

        assert_eq!(rgba.len(), (TRAY_WAVEFORM_ICON_SIZE * TRAY_WAVEFORM_ICON_SIZE * 4) as usize);
        assert_eq!(rgba_pixel(&rgba, TRAY_WAVEFORM_ICON_SIZE, 4, 8), [0, 0, 0, 255]);
        assert_eq!(rgba_pixel(&rgba, TRAY_WAVEFORM_ICON_SIZE, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn tray_waveform_icon_draws_recording_dot() {
        let rgba = tray_waveform_icon_rgba(true);

        assert_eq!(rgba_pixel(&rgba, TRAY_WAVEFORM_ICON_SIZE, 15, 3), [255, 59, 48, 255]);
    }

    #[test]
    fn global_shortcuts_map_to_frontend_commands() {
        let record = Shortcut::new(Some(Modifiers::ALT | Modifiers::SUPER), Code::KeyR);
        let overlay = Shortcut::new(Some(Modifiers::ALT | Modifiers::SUPER), Code::KeyL);
        let control_record = Shortcut::new(Some(Modifiers::ALT | Modifiers::CONTROL), Code::KeyR);
        let unrelated = Shortcut::new(Some(Modifiers::ALT | Modifiers::SUPER), Code::KeyP);

        assert_eq!(
            shortcut_command(&record),
            Some(TRAY_COMMAND_TOGGLE_RECORDING)
        );
        assert_eq!(
            shortcut_command(&overlay),
            Some(TRAY_COMMAND_TOGGLE_OVERLAY)
        );
        assert_eq!(
            shortcut_command(&control_record),
            Some(TRAY_COMMAND_TOGGLE_RECORDING)
        );
        assert_eq!(shortcut_command(&unrelated), None);
    }
}
