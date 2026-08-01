use crate::whisper_engine_protocol::WhisperEngineInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperBackendConfig {
    pub model_path: String,
    pub language: String,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperBackendError {
    pub message: String,
}

pub trait WhisperBackend {
    fn load_model(&mut self, config: WhisperBackendConfig) -> Result<(), WhisperBackendError>;
}

#[derive(Debug, Default)]
pub struct NoopWhisperBackend {
    pub loaded_config: Option<WhisperBackendConfig>,
}

impl WhisperBackend for NoopWhisperBackend {
    fn load_model(&mut self, config: WhisperBackendConfig) -> Result<(), WhisperBackendError> {
        self.loaded_config = Some(config);
        Ok(())
    }
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
