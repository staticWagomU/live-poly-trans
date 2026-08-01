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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WhisperEngineOutput {
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
}
