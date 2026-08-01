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

pub fn engine_input_line(input: &WhisperEngineInput) -> Result<String, serde_json::Error> {
    serde_json::to_string(input)
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
}
