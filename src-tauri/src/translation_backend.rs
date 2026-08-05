use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    InvalidResponse(String),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
