use crate::whisper_engine_protocol::WhisperEngineInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperBackendConfig {
    pub model_path: String,
    pub language: String,
    pub sample_rate: u32,
}

impl TryFrom<WhisperEngineInput> for WhisperBackendConfig {
    type Error = &'static str;

    fn try_from(input: WhisperEngineInput) -> Result<Self, Self::Error> {
        match input {
            WhisperEngineInput::Config {
                model_path,
                language,
                sample_rate,
            } => Ok(Self {
                model_path,
                language,
                sample_rate,
            }),
            _ => Err("not a config input"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whisper_engine_protocol::WhisperEngineInput;

    #[test]
    fn builds_backend_config_from_protocol_config_input() {
        let config = WhisperBackendConfig::try_from(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        })
        .unwrap();

        assert_eq!(
            config,
            WhisperBackendConfig {
                model_path: "/models/ggml-base.bin".to_string(),
                language: "auto".to_string(),
                sample_rate: 16_000,
            }
        );
    }
}
