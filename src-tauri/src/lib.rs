use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};

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
) -> Result<Option<(String, String)>, String> {
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
    Ok(Some((model.to_string(), cli.display().to_string())))
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
        let pending: Arc<Mutex<HashMap<String, mpsc::Sender<AiServerResponse>>>> =
            Arc::default();

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
                        response.error.unwrap_or_else(|| "unknown AI error".to_string()),
                    ))
                }
            }
            Err(_) => {
                if let Ok(mut guard) = self.pending.lock() {
                    guard.remove(&id);
                }
                Err(AiRequestError::Transport("AI request timed out".to_string()))
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

// MARK: recordings

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMeta {
    pub id: String,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "endedAt", default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
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

// MARK: recording session control

pub fn start_recording_control_line(dir: &str) -> String {
    serde_json::json!({ "cmd": "start-recording", "dir": dir }).to_string()
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
                return Err(
                    "no capture is running; recording needs live transcription".to_string()
                );
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
                        Some(
                            next_recording_file_path(&dir, &stream)
                                .to_string_lossy()
                                .into_owned(),
                        ),
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
                    .map(|(model_path, cli_path)| WhisperEngineConfig {
                        model_path,
                        cli_path,
                    })
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
                read_json_lines(app.clone(), stdout, session_id.clone(), control_ready.clone());
            }

            if let Some(stderr) = child.stderr.take() {
                read_stderr(app.clone(), stderr);
            }

            children
                .lock()
                .map_err(|error| error.to_string())?
                .insert(
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
    ) -> Result<CreatedRecording, String> {
        let children = state.children.clone();

        tauri::async_runtime::spawn_blocking(move || {
            wait_for_control_ready(&children, CONTROL_READY_TIMEOUT)?;
            let created = create_recording_blocking(&app)?;

            let line = start_recording_control_line(&created.dir);
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
    pub async fn delete_recording(app: AppHandle, id: String) -> Result<(), String> {
        tauri::async_runtime::spawn_blocking(move || {
            let root = recordings_dir(&app)?;
            delete_recording_dir(&root, &id)
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
                files,
            });
        }

        recordings.sort_by(|left, right| right.started_at.cmp(&left.started_at));
        Ok(recordings)
    }

    #[tauri::command]
    pub async fn read_recording_transcript(app: AppHandle, id: String) -> Result<Vec<Value>, String> {
        // An 8-hour session's jsonl runs to tens of MB; parsing it on the
        // main thread freezes the whole window.
        tauri::async_runtime::spawn_blocking(move || read_recording_transcript_blocking(&app, &id))
            .await
            .map_err(|error| error.to_string())?
    }

    fn read_recording_transcript_blocking(app: &AppHandle, id: &str) -> Result<Vec<Value>, String> {
        let dir = recording_dir_for(app, id)?;
        let Ok(entries) = fs::read_dir(&dir) else {
            return Ok(Vec::new());
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

        Ok(events)
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
                            return Err(
                                String::from_utf8_lossy(&output.stderr).trim().to_owned()
                            );
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
}

pub fn run() {
    tauri::Builder::default()
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
            commands::ai_ask,
            commands::create_recording,
            commands::finalize_recording,
            commands::start_recording_session,
            commands::stop_recording_session,
            commands::delete_recording,
            commands::list_recordings,
            commands::read_recording_transcript,
            commands::recording_waveform,
            commands::export_recording
        ])
        .setup(|app| {
            let _ = app.get_webview_window("main");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running LivePolyTrans")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                shutdown_capture_on_exit(app);
            }
        });
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
    fn recording_control_lines_match_the_helper_protocol() {
        assert_eq!(
            start_recording_control_line("/tmp/rec-1"),
            r#"{"cmd":"start-recording","dir":"/tmp/rec-1"}"#
        );
        assert_eq!(stop_recording_control_line(), r#"{"cmd":"stop-recording"}"#);
    }

    #[test]
    fn start_recording_control_line_escapes_special_characters() {
        let line = start_recording_control_line(r#"/tmp/we"ird dir"#);
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
        let root = std::env::temp_dir().join(format!(
            "lpt-close-open-recordings-{}",
            std::process::id()
        ));
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
            },
        )
        .unwrap();
        write_recording_meta(
            &closed_dir,
            &RecordingMeta {
                id: "rec-closed".to_string(),
                started_at: "2026-07-31T09:00:00+09:00".to_string(),
                ended_at: Some("2026-07-31T09:30:00+09:00".to_string()),
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
    fn delete_recording_dir_removes_the_directory_with_contents() {
        let root = std::env::temp_dir().join(format!("lpt-delete-recording-{}", std::process::id()));
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
            }),
        );

        let tail: Vec<&str> = args.iter().rev().take(6).rev().map(String::as_str).collect();
        assert_eq!(
            tail,
            vec![
                "--transcription-engine",
                "whisper",
                "--whisper-model",
                "/models/ggml-large-v3-turbo.bin",
                "--whisper-cli",
                "/opt/homebrew/bin/whisper-cli",
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
    fn readable_timestamp_is_filename_safe() {
        let timestamp = readable_timestamp();
        assert_eq!(timestamp.len(), "20260704-120000".len());
        assert!(!timestamp.contains(':'));
        assert!(!timestamp.contains('/'));
    }
}
