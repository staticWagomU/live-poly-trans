use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WhisperEngineInput {
    #[serde(rename_all = "camelCase")]
    Config {
        model_path: String,
        language: String,
        sample_rate: u32,
    },
    #[serde(rename_all = "camelCase")]
    Audio {
        stream: String,
        seq: u64,
        timestamp_ms: i64,
        pcm16_base64: String,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WhisperEngineOutput {
    #[serde(rename_all = "camelCase")]
    Status { state: String },
    #[serde(rename_all = "camelCase")]
    Transcript {
        stream: String,
        segment_id: String,
        text: String,
        is_final: bool,
        start_ms: i64,
        duration_ms: i64,
        language: String,
        confidence: Option<f64>,
    },
}

pub fn engine_input_line(input: &WhisperEngineInput) -> Result<String, serde_json::Error> {
    serde_json::to_string(input)
}

pub fn parse_engine_input_line(line: &str) -> Result<WhisperEngineInput, serde_json::Error> {
    serde_json::from_str(line)
}

pub fn engine_output_line(output: &WhisperEngineOutput) -> Result<String, serde_json::Error> {
    serde_json::to_string(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_config_input_as_camel_case_json_line() {
        let line = engine_input_line(&WhisperEngineInput::Config {
            model_path: "/models/ggml-large-v3-turbo.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        })
        .unwrap();

        assert_eq!(
            line,
            r#"{"type":"config","modelPath":"/models/ggml-large-v3-turbo.bin","language":"auto","sampleRate":16000}"#
        );
    }

    #[test]
    fn serializes_transcript_output_as_camel_case_json_line() {
        let line = engine_output_line(&WhisperEngineOutput::Transcript {
            stream: "mic".to_string(),
            segment_id: "mic-1200".to_string(),
            text: "hello".to_string(),
            is_final: false,
            start_ms: 1200,
            duration_ms: 900,
            language: "en".to_string(),
            confidence: Some(0.82),
        })
        .unwrap();

        assert_eq!(
            line,
            r#"{"type":"transcript","stream":"mic","segmentId":"mic-1200","text":"hello","isFinal":false,"startMs":1200,"durationMs":900,"language":"en","confidence":0.82}"#
        );
    }

    #[test]
    fn parses_shutdown_input_line() {
        let input = parse_engine_input_line(r#"{"type":"shutdown"}"#).unwrap();

        assert_eq!(input, WhisperEngineInput::Shutdown);
    }

    #[test]
    fn serializes_status_output_as_json_line() {
        let line = engine_output_line(&WhisperEngineOutput::Status {
            state: "ready".to_string(),
        })
        .unwrap();

        assert_eq!(line, r#"{"type":"status","state":"ready"}"#);
    }

    #[test]
    fn parses_audio_input_line() {
        let input = parse_engine_input_line(
            r#"{"type":"audio","stream":"mic","seq":7,"timestampMs":1234,"pcm16Base64":"AAE="}"#,
        )
        .unwrap();

        assert_eq!(
            input,
            WhisperEngineInput::Audio {
                stream: "mic".to_string(),
                seq: 7,
                timestamp_ms: 1234,
                pcm16_base64: "AAE=".to_string()
            }
        );
    }
}
