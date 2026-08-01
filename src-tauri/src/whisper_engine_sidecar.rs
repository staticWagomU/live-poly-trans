use crate::whisper_engine_backend::{NoopWhisperBackend, WhisperBackend, WhisperBackendConfig};
use crate::whisper_engine_audio::{decode_pcm16_base64, PcmRingBuffer};
use crate::whisper_engine_protocol::{WhisperEngineInput, WhisperEngineOutput};
use std::collections::HashMap;

pub const DEFAULT_RING_BUFFER_SAMPLES: usize = 16_000 * 30;

#[derive(Debug, Clone, PartialEq)]
pub enum SidecarAction {
    Continue(Option<WhisperEngineOutput>),
    Shutdown,
}

pub struct WhisperEngineSidecar<B = NoopWhisperBackend> {
    capacity_samples: usize,
    buffers: HashMap<String, PcmRingBuffer>,
    backend: B,
}

impl WhisperEngineSidecar {
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_RING_BUFFER_SAMPLES)
    }

    pub fn new(capacity_samples: usize) -> Self {
        Self {
            capacity_samples,
            buffers: HashMap::new(),
            backend: NoopWhisperBackend::default(),
        }
    }
}

impl<B: WhisperBackend> WhisperEngineSidecar<B> {
    pub fn with_backend(capacity_samples: usize, backend: B) -> Self {
        Self {
            capacity_samples,
            buffers: HashMap::new(),
            backend,
        }
    }

    pub fn handle_input(&mut self, input: WhisperEngineInput) -> SidecarAction {
        match input {
            WhisperEngineInput::Config { .. } => {
                let config = WhisperBackendConfig::try_from(input)
                    .expect("config arm only passes config input");
                match self.backend.load_model(config) {
                    Ok(()) => SidecarAction::Continue(Some(WhisperEngineOutput::Status {
                        state: "ready".to_string(),
                    })),
                    Err(error) => SidecarAction::Continue(Some(WhisperEngineOutput::Error {
                        message: error.message,
                        fatal: true,
                    })),
                }
            }
            WhisperEngineInput::Audio {
                stream,
                pcm16_base64,
                ..
            } => {
                let Ok(samples) = decode_pcm16_base64(&pcm16_base64) else {
                    return SidecarAction::Continue(Some(WhisperEngineOutput::Error {
                        message: "invalid pcm16 audio payload".to_string(),
                        fatal: false,
                    }));
                };

                self.buffers
                    .entry(stream)
                    .or_insert_with(|| PcmRingBuffer::new(self.capacity_samples))
                    .push(&samples);
                SidecarAction::Continue(None)
            }
            input => handle_engine_input(input),
        }
    }

    pub fn samples(&self, stream: &str) -> Option<&[i16]> {
        self.buffers.get(stream).map(PcmRingBuffer::samples)
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
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

    #[test]
    fn audio_input_continues_without_output_until_backend_is_connected() {
        assert_eq!(
            handle_engine_input(WhisperEngineInput::Audio {
                stream: "mic".to_string(),
                seq: 1,
                timestamp_ms: 0,
                pcm16_base64: "AAE=".to_string()
            }),
            SidecarAction::Continue(None)
        );
    }

    #[test]
    fn flush_input_continues_without_output_until_backend_is_connected() {
        assert_eq!(
            handle_engine_input(WhisperEngineInput::Flush {
                stream: "speaker".to_string()
            }),
            SidecarAction::Continue(None)
        );
    }

    #[test]
    fn audio_input_appends_decoded_samples_to_the_stream_buffer() {
        let mut sidecar = WhisperEngineSidecar::new(4);

        let action = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: "AQD//w==".to_string(),
        });

        assert_eq!(action, SidecarAction::Continue(None));
        assert_eq!(sidecar.samples("mic"), Some(&[1, -1][..]));
    }

    #[test]
    fn malformed_audio_input_reports_a_non_fatal_error() {
        let mut sidecar = WhisperEngineSidecar::new(4);

        let action = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: "not valid base64".to_string(),
        });

        assert_eq!(
            action,
            SidecarAction::Continue(Some(WhisperEngineOutput::Error {
                message: "invalid pcm16 audio payload".to_string(),
                fatal: false
            }))
        );
    }

    #[derive(Default)]
    struct FakeBackend {
        loaded: Vec<crate::whisper_engine_backend::WhisperBackendConfig>,
    }

    impl crate::whisper_engine_backend::WhisperBackend for FakeBackend {
        fn load_model(
            &mut self,
            config: crate::whisper_engine_backend::WhisperBackendConfig,
        ) -> Result<(), crate::whisper_engine_backend::WhisperBackendError> {
            self.loaded.push(config);
            Ok(())
        }
    }

    #[test]
    fn config_input_loads_the_backend_model_once() {
        let backend = FakeBackend::default();
        let mut sidecar = WhisperEngineSidecar::with_backend(4, backend);

        let action = sidecar.handle_input(WhisperEngineInput::Config {
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
        assert_eq!(sidecar.backend().loaded.len(), 1);
        assert_eq!(sidecar.backend().loaded[0].model_path, "/models/ggml-base.bin");
    }
}
