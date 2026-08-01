use crate::whisper_engine_protocol::{WhisperEngineInput, WhisperEngineOutput};

#[derive(Debug, Clone, PartialEq)]
pub enum SidecarAction {
    Continue(Option<WhisperEngineOutput>),
}

pub fn handle_engine_input(input: WhisperEngineInput) -> SidecarAction {
    match input {
        WhisperEngineInput::Config { .. } => {
            SidecarAction::Continue(Some(WhisperEngineOutput::Status {
                state: "ready".to_string(),
            }))
        }
        WhisperEngineInput::Shutdown => SidecarAction::Continue(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whisper_engine_protocol::{WhisperEngineInput, WhisperEngineOutput};

    #[test]
    fn config_input_reports_ready_status() {
        let action = handle_engine_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        assert_eq!(
            action,
            SidecarAction::Continue(Some(WhisperEngineOutput::Status {
                state: "ready".to_string()
            }))
        );
    }
}
