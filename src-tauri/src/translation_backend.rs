use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TranslationBackendKind {
    Apple,
    DeepL,
    Ollama,
}

pub trait TranslationBackend {
    fn kind(&self) -> TranslationBackendKind;
    fn translate(&self, request: &TranslationRequest) -> Result<Option<String>, TranslationError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRequest {
    pub text: String,
    pub source_language: String,
    pub target_language: String,
}

impl TranslationRequest {
    pub fn new(
        text: impl Into<String>,
        source_language: impl Into<String>,
        target_language: impl Into<String>,
    ) -> Option<Self> {
        let text = text.into().trim().to_string();
        let source_language = source_language.into();
        let target_language = target_language.into();
        if text.is_empty() || source_language == target_language {
            return None;
        }

        Some(Self {
            text,
            source_language,
            target_language,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationError {
    MissingCredential(String),
    Network(String),
    Keychain(String),
    InvalidResponse(String),
}

impl fmt::Display for TranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranslationError::MissingCredential(message)
            | TranslationError::Network(message)
            | TranslationError::Keychain(message)
            | TranslationError::InvalidResponse(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for TranslationError {}

pub const DEEPL_API_ENDPOINT: &str = "https://api.deepl.com/v2/translate";
pub const DEEPL_FREE_API_ENDPOINT: &str = "https://api-free.deepl.com/v2/translate";
pub const KEYCHAIN_SERVICE: &str = "live-poly-trans";
pub const DEEPL_API_KEY_ACCOUNT: &str = "deepl-api-key";

pub fn keychain_find_password_args(service: &str, account: &str) -> Vec<String> {
    vec![
        "find-generic-password".to_string(),
        "-s".to_string(),
        service.to_string(),
        "-a".to_string(),
        account.to_string(),
        "-w".to_string(),
    ]
}

pub fn keychain_write_password_args(service: &str, account: &str, password: &str) -> Vec<String> {
    vec![
        "add-generic-password".to_string(),
        "-U".to_string(),
        "-s".to_string(),
        service.to_string(),
        "-a".to_string(),
        account.to_string(),
        "-w".to_string(),
        password.to_string(),
    ]
}

pub fn keychain_delete_password_args(service: &str, account: &str) -> Vec<String> {
    vec![
        "delete-generic-password".to_string(),
        "-s".to_string(),
        service.to_string(),
        "-a".to_string(),
        account.to_string(),
    ]
}

pub fn read_deepl_api_key_from_keychain() -> Result<Option<String>, TranslationError> {
    read_keychain_password(KEYCHAIN_SERVICE, DEEPL_API_KEY_ACCOUNT)
}

pub fn write_deepl_api_key_to_keychain(api_key: &str) -> Result<(), TranslationError> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        delete_deepl_api_key_from_keychain()
    } else {
        write_keychain_password(KEYCHAIN_SERVICE, DEEPL_API_KEY_ACCOUNT, trimmed)
    }
}

pub fn delete_deepl_api_key_from_keychain() -> Result<(), TranslationError> {
    delete_keychain_password(KEYCHAIN_SERVICE, DEEPL_API_KEY_ACCOUNT)
}

fn read_keychain_password(
    service: &'static str,
    account: &'static str,
) -> Result<Option<String>, TranslationError> {
    let output = Command::new("security")
        .args(keychain_find_password_args(service, account))
        .output()
        .map_err(|error| TranslationError::Keychain(error.to_string()))?;
    if output.status.success() {
        let password = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok((!password.is_empty()).then_some(password));
    }
    if keychain_output_is_not_found(&output) {
        return Ok(None);
    }
    Err(TranslationError::Keychain(keychain_error_message(&output)))
}

fn write_keychain_password(
    service: &'static str,
    account: &'static str,
    password: &str,
) -> Result<(), TranslationError> {
    let output = Command::new("security")
        .args(keychain_write_password_args(service, account, password))
        .output()
        .map_err(|error| TranslationError::Keychain(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(TranslationError::Keychain(keychain_error_message(&output)))
    }
}

fn delete_keychain_password(
    service: &'static str,
    account: &'static str,
) -> Result<(), TranslationError> {
    let output = Command::new("security")
        .args(keychain_delete_password_args(service, account))
        .output()
        .map_err(|error| TranslationError::Keychain(error.to_string()))?;
    if output.status.success() || keychain_output_is_not_found(&output) {
        Ok(())
    } else {
        Err(TranslationError::Keychain(keychain_error_message(&output)))
    }
}

fn keychain_output_is_not_found(output: &std::process::Output) -> bool {
    output.status.code() == Some(44)
        || keychain_error_message(output)
            .to_ascii_lowercase()
            .contains("could not be found")
}

fn keychain_error_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("security exited with {}", output.status)
    } else {
        stderr
    }
}

pub fn deepl_language_code(language: &str) -> String {
    let upper = language.replace('_', "-").to_ascii_uppercase();
    match upper.as_str() {
        "JA-JP" => "JA".to_string(),
        "EN" | "EN-US" | "EN-GB" => upper,
        _ => upper
            .split('-')
            .next()
            .filter(|code| !code.is_empty())
            .unwrap_or(&upper)
            .to_string(),
    }
}

pub fn deepl_form_fields(request: &TranslationRequest) -> Vec<(&'static str, String)> {
    vec![
        ("text", request.text.clone()),
        ("source_lang", deepl_language_code(&request.source_language)),
        ("target_lang", deepl_language_code(&request.target_language)),
    ]
}

pub fn deepl_json_request(request: &TranslationRequest) -> Value {
    json!({
        "text": [request.text.clone()],
        "source_lang": deepl_language_code(&request.source_language),
        "target_lang": deepl_language_code(&request.target_language),
    })
}

pub fn deepl_endpoint_for_key(api_key: &str) -> &'static str {
    if api_key.trim().ends_with(":fx") {
        DEEPL_FREE_API_ENDPOINT
    } else {
        DEEPL_API_ENDPOINT
    }
}

pub fn parse_deepl_response(body: &str) -> Result<String, TranslationError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| TranslationError::InvalidResponse(error.to_string()))?;
    value
        .get("translations")
        .and_then(Value::as_array)
        .and_then(|translations| translations.first())
        .and_then(|translation| translation.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| TranslationError::InvalidResponse("missing translations[0].text".into()))
}

#[derive(Debug, Clone)]
pub struct DeepLHttpBackend {
    endpoint: String,
    api_key: String,
}

impl DeepLHttpBackend {
    pub fn new(api_key: impl Into<String>) -> Self {
        let api_key = api_key.into();
        Self {
            endpoint: deepl_endpoint_for_key(&api_key).to_string(),
            api_key,
        }
    }

    pub fn with_endpoint(api_key: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
        }
    }
}

impl TranslationBackend for DeepLHttpBackend {
    fn kind(&self) -> TranslationBackendKind {
        TranslationBackendKind::DeepL
    }

    fn translate(&self, request: &TranslationRequest) -> Result<Option<String>, TranslationError> {
        let api_key = self.api_key.trim();
        if api_key.is_empty() {
            return Err(TranslationError::MissingCredential(
                "DeepL API key is not configured".to_string(),
            ));
        }

        let auth = format!("DeepL-Auth-Key {api_key}");
        let response = ureq::post(&self.endpoint)
            .set("Authorization", &auth)
            .set("Content-Type", "application/json")
            .send_json(deepl_json_request(request))
            .map_err(http_error)?;
        let body = response
            .into_string()
            .map_err(|error| TranslationError::Network(error.to_string()))?;
        parse_deepl_response(&body).map(Some)
    }
}

pub fn ollama_chat_request(model: &str, request: &TranslationRequest) -> Value {
    json!({
        "model": model,
        "stream": false,
        "messages": [
            {
                "role": "system",
                "content": format!(
                    "Translate from {} to {}. Return only the translated text.",
                    request.source_language, request.target_language
                )
            },
            {
                "role": "user",
                "content": request.text
            }
        ]
    })
}

pub fn ollama_chat_url(endpoint: &str) -> String {
    format!("{}/api/chat", endpoint.trim_end_matches('/'))
}

pub fn parse_ollama_chat_response(body: &str) -> Result<String, TranslationError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| TranslationError::InvalidResponse(error.to_string()))?;
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| TranslationError::InvalidResponse("missing message.content".into()))
}

#[derive(Debug, Clone)]
pub struct OllamaHttpBackend {
    endpoint: String,
    model: String,
}

impl OllamaHttpBackend {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
        }
    }
}

impl TranslationBackend for OllamaHttpBackend {
    fn kind(&self) -> TranslationBackendKind {
        TranslationBackendKind::Ollama
    }

    fn translate(&self, request: &TranslationRequest) -> Result<Option<String>, TranslationError> {
        let model = self.model.trim();
        if model.is_empty() {
            return Err(TranslationError::MissingCredential(
                "Ollama model is not configured".to_string(),
            ));
        }

        let response = ureq::post(&ollama_chat_url(&self.endpoint))
            .set("Content-Type", "application/json")
            .send_json(ollama_chat_request(model, request))
            .map_err(http_error)?;
        let body = response
            .into_string()
            .map_err(|error| TranslationError::Network(error.to_string()))?;
        parse_ollama_chat_response(&body).map(Some)
    }
}

fn http_error(error: ureq::Error) -> TranslationError {
    match error {
        ureq::Error::Status(code, response) => {
            let body = response.into_string().unwrap_or_default();
            TranslationError::Network(format!("HTTP {code}: {body}"))
        }
        ureq::Error::Transport(error) => TranslationError::Network(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    fn serve_json_once(body: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();

            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        request.extend_from_slice(&buffer[..count]);
                        if request.windows(4).any(|window| window == b"\r\n\r\n") {
                            let text = String::from_utf8_lossy(&request);
                            if let Some(length) = content_length(&text) {
                                let header_end = text.find("\r\n\r\n").unwrap() + 4;
                                if request.len() >= header_end + length {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&request).to_string()
        });
        (url, handle)
    }

    fn content_length(request: &str) -> Option<usize> {
        request.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
    }

    #[test]
    fn translation_request_rejects_blank_or_same_language_input() {
        assert!(TranslationRequest::new("  ", "ja-JP", "en-US").is_none());
        assert!(TranslationRequest::new("hello", "ja-JP", "ja-JP").is_none());
        assert_eq!(
            TranslationRequest::new(" hello ", "en-US", "ja-JP").unwrap(),
            TranslationRequest {
                text: "hello".to_string(),
                source_language: "en-US".to_string(),
                target_language: "ja-JP".to_string(),
            }
        );
    }

    #[test]
    fn deepl_form_fields_use_deepl_language_codes() {
        let request = TranslationRequest::new("hello", "en-US", "ja-JP").unwrap();

        assert_eq!(
            deepl_form_fields(&request),
            vec![
                ("text", "hello".to_string()),
                ("source_lang", "EN-US".to_string()),
                ("target_lang", "JA".to_string()),
            ]
        );
    }

    #[test]
    fn builds_deepl_json_request_and_endpoint() {
        let request = TranslationRequest::new("hello", "en-US", "ja-JP").unwrap();

        assert_eq!(
            deepl_json_request(&request),
            json!({
                "text": ["hello"],
                "source_lang": "EN-US",
                "target_lang": "JA",
            })
        );
        assert_eq!(deepl_endpoint_for_key("secret"), DEEPL_API_ENDPOINT);
        assert_eq!(deepl_endpoint_for_key("secret:fx"), DEEPL_FREE_API_ENDPOINT);
    }

    #[test]
    fn builds_keychain_security_command_args() {
        assert_eq!(
            keychain_find_password_args(KEYCHAIN_SERVICE, DEEPL_API_KEY_ACCOUNT),
            vec![
                "find-generic-password",
                "-s",
                "live-poly-trans",
                "-a",
                "deepl-api-key",
                "-w",
            ]
        );
        assert_eq!(
            keychain_write_password_args(KEYCHAIN_SERVICE, DEEPL_API_KEY_ACCOUNT, "secret"),
            vec![
                "add-generic-password",
                "-U",
                "-s",
                "live-poly-trans",
                "-a",
                "deepl-api-key",
                "-w",
                "secret",
            ]
        );
        assert_eq!(
            keychain_delete_password_args(KEYCHAIN_SERVICE, DEEPL_API_KEY_ACCOUNT),
            vec![
                "delete-generic-password",
                "-s",
                "live-poly-trans",
                "-a",
                "deepl-api-key",
            ]
        );
    }

    #[test]
    fn parses_deepl_response_text() {
        assert_eq!(
            parse_deepl_response(r#"{"translations":[{"text":"こんにちは"}]}"#).unwrap(),
            "こんにちは"
        );
        assert!(parse_deepl_response(r#"{"translations":[]}"#).is_err());
    }

    #[test]
    fn builds_ollama_chat_request_without_streaming() {
        let request = TranslationRequest::new("hello", "en-US", "ja-JP").unwrap();

        assert_eq!(
            ollama_chat_request("llama3.1", &request),
            json!({
                "model": "llama3.1",
                "stream": false,
                "messages": [
                    {
                        "role": "system",
                        "content": "Translate from en-US to ja-JP. Return only the translated text."
                    },
                    {
                        "role": "user",
                        "content": "hello"
                    }
                ]
            })
        );
    }

    #[test]
    fn parses_ollama_chat_response_text() {
        assert_eq!(
            parse_ollama_chat_response(r#"{"message":{"content":"  こんにちは\n"}}"#).unwrap(),
            "こんにちは"
        );
        assert!(parse_ollama_chat_response(r#"{"message":{"content":" "}}"#).is_err());
    }

    #[test]
    fn deepl_backend_posts_translation_request() {
        let (base_url, handle) = serve_json_once(r#"{"translations":[{"text":"こんにちは"}]}"#);
        let backend =
            DeepLHttpBackend::with_endpoint("secret:fx", format!("{base_url}/v2/translate"));
        let request = TranslationRequest::new("hello", "en-US", "ja-JP").unwrap();

        assert_eq!(
            backend.translate(&request).unwrap(),
            Some("こんにちは".to_string())
        );
        let raw_request = handle.join().unwrap();
        assert!(raw_request.starts_with("POST /v2/translate HTTP/1.1"));
        assert!(raw_request.contains("Authorization: DeepL-Auth-Key secret:fx"));
        assert!(raw_request.contains(r#""text":["hello"]"#));
        assert!(raw_request.contains(r#""source_lang":"EN-US""#));
        assert!(raw_request.contains(r#""target_lang":"JA""#));
    }

    #[test]
    fn ollama_backend_posts_chat_request() {
        let (base_url, handle) = serve_json_once(r#"{"message":{"content":" こんにちは "}}"#);
        let backend = OllamaHttpBackend::new(base_url, "llama3.1");
        let request = TranslationRequest::new("hello", "en-US", "ja-JP").unwrap();

        assert_eq!(
            backend.translate(&request).unwrap(),
            Some("こんにちは".to_string())
        );
        let raw_request = handle.join().unwrap();
        assert!(raw_request.starts_with("POST /api/chat HTTP/1.1"));
        assert!(raw_request.contains(r#""model":"llama3.1""#));
        assert!(raw_request.contains(r#""stream":false"#));
        assert!(raw_request.contains("Translate from en-US to ja-JP"));
    }
}
