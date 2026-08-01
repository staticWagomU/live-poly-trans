use crate::whisper_engine_protocol::{WhisperEngineInput, WhisperEngineOutput};

#[derive(Debug, Clone, PartialEq)]
pub enum SidecarAction {
    Continue(Option<WhisperEngineOutput>),
    Shutdown,
}

pub fn handle_engine_input(input: WhisperEngineInput) -> SidecarAction {
    match input {
        WhisperEngineInput::Config { .. } => {
            SidecarAction::Continue(Some(WhisperEngineOutput::Status {
                state: "ready".to_string(),
            }))
        }
        WhisperEngineInput::Audio { .. } => SidecarAction::Continue(None),
        WhisperEngineInput::Flush { .. } => SidecarAction::Continue(None),
        WhisperEngineInput::Shutdown => SidecarAction::Shutdown,
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

    #[test]
    fn shutdown_input_stops_the_sidecar_loop() {
        assert_eq!(
            handle_engine_input(WhisperEngineInput::Shutdown),
            SidecarAction::Shutdown
        );
    }
}
