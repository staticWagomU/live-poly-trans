use crate::whisper_engine_audio::{decode_pcm16_base64, pcm16_to_f32, PcmRingBuffer};
use crate::whisper_engine_backend::{NoopWhisperBackend, WhisperBackend, WhisperBackendConfig};
use crate::whisper_engine_protocol::{WhisperEngineInput, WhisperEngineOutput};
use crate::whisper_engine_scheduler::{
    RollingTranscriptionDecision, RollingTranscriptionScheduler,
};
use crate::whisper_engine_stabilization::{collapse_exact_repeated_text, PartialStabilizer};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub const DEFAULT_RING_BUFFER_SAMPLES: usize = 16_000 * 30;
pub const DEFAULT_ROLLING_STEP_SAMPLES: usize = 16_000 * 750 / 1000;
pub const DEFAULT_MAX_ROLLING_BACKLOG_STEPS: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum SidecarAction {
    Continue(Vec<WhisperEngineOutput>),
    Shutdown,
}

impl SidecarAction {
    pub fn none() -> Self {
        SidecarAction::Continue(Vec::new())
    }

    pub fn output(output: WhisperEngineOutput) -> Self {
        SidecarAction::Continue(vec![output])
    }

    pub fn outputs(outputs: Vec<WhisperEngineOutput>) -> Self {
        SidecarAction::Continue(outputs)
    }
}

pub struct WhisperEngineSidecar<B = NoopWhisperBackend> {
    capacity_samples: usize,
    buffers: HashMap<String, PcmRingBuffer>,
    schedulers: HashMap<String, RollingTranscriptionScheduler>,
    stabilizers: HashMap<String, PartialStabilizer>,
    first_audio_at: HashMap<String, Instant>,
    first_partial_emitted: HashSet<String>,
    backend: B,
    sample_rate: u32,
    rolling_step_samples: usize,
}

impl WhisperEngineSidecar {
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_RING_BUFFER_SAMPLES)
    }

    pub fn new(capacity_samples: usize) -> Self {
        Self {
            capacity_samples,
            buffers: HashMap::new(),
            schedulers: HashMap::new(),
            stabilizers: HashMap::new(),
            first_audio_at: HashMap::new(),
            first_partial_emitted: HashSet::new(),
            backend: NoopWhisperBackend::default(),
            sample_rate: 16_000,
            rolling_step_samples: DEFAULT_ROLLING_STEP_SAMPLES,
        }
    }
}

impl<B: WhisperBackend> WhisperEngineSidecar<B> {
    pub fn with_backend(capacity_samples: usize, backend: B) -> Self {
        Self::with_backend_and_step_samples(capacity_samples, DEFAULT_ROLLING_STEP_SAMPLES, backend)
    }

    pub fn with_backend_and_step_samples(
        capacity_samples: usize,
        rolling_step_samples: usize,
        backend: B,
    ) -> Self {
        Self {
            capacity_samples,
            buffers: HashMap::new(),
            schedulers: HashMap::new(),
            stabilizers: HashMap::new(),
            first_audio_at: HashMap::new(),
            first_partial_emitted: HashSet::new(),
            backend,
            sample_rate: 16_000,
            rolling_step_samples,
        }
    }

    pub fn handle_input(&mut self, input: WhisperEngineInput) -> SidecarAction {
        match input {
            WhisperEngineInput::Config { .. } => {
                let config = WhisperBackendConfig::try_from(input)
                    .expect("config arm only passes config input");
                self.sample_rate = config.sample_rate;
                match self.backend.load_model(config) {
                    Ok(()) => SidecarAction::output(WhisperEngineOutput::Status {
                        state: "ready".to_string(),
                    }),
                    Err(error) => SidecarAction::output(WhisperEngineOutput::Error {
                        message: error.message,
                        fatal: true,
                    }),
                }
            }
            WhisperEngineInput::Audio {
                stream,
                pcm16_base64,
                ..
            } => {
                let Ok(samples) = decode_pcm16_base64(&pcm16_base64) else {
                    return SidecarAction::output(WhisperEngineOutput::Error {
                        message: "invalid pcm16 audio payload".to_string(),
                        fatal: false,
                    });
                };

                self.first_audio_at
                    .entry(stream.clone())
                    .or_insert_with(Instant::now);
                self.buffers
                    .entry(stream.clone())
                    .or_insert_with(|| PcmRingBuffer::new(self.capacity_samples))
                    .push(&samples);
                let Some(buffer) = self.buffers.get(&stream) else {
                    return SidecarAction::none();
                };
                let scheduler = self.schedulers.entry(stream.clone()).or_insert_with(|| {
                    RollingTranscriptionScheduler::new(self.rolling_step_samples)
                });
                match scheduler.observe_total_samples_with_backlog_limit(
                    buffer.samples().len(),
                    DEFAULT_MAX_ROLLING_BACKLOG_STEPS,
                ) {
                    RollingTranscriptionDecision::Wait
                    | RollingTranscriptionDecision::DropBacklog => {
                        return SidecarAction::none();
                    }
                    RollingTranscriptionDecision::Transcribe => {}
                }
                return self.transcribe_buffer(&stream, false, "rolling");
            }
            WhisperEngineInput::Flush { stream } => {
                let action = self.transcribe_buffer(&stream, true, "flush");
                if let Some(stabilizer) = self.stabilizers.get_mut(&stream) {
                    stabilizer.reset();
                }
                return action;
            }
            input => handle_engine_input(input),
        }
    }

    fn transcribe_buffer(
        &mut self,
        stream: &str,
        is_final: bool,
        segment_suffix: &str,
    ) -> SidecarAction {
        let Some(buffer) = self.buffers.get(stream) else {
            return SidecarAction::none();
        };
        let samples = pcm16_to_f32(buffer.samples());
        let started_at = Instant::now();
        match self.backend.transcribe(&samples) {
            Ok(Some(mut transcript)) => {
                let elapsed_ms = started_at.elapsed().as_secs_f64() * 1000.0;
                transcript.text = collapse_exact_repeated_text(&transcript.text);
                if !is_final {
                    let stabilizer = self.stabilizers.entry(stream.to_string()).or_default();
                    let Some(stable_text) = stabilizer.observe(&transcript.text) else {
                        return SidecarAction::none();
                    };
                    transcript.text = stable_text;
                }

                let mut outputs = vec![WhisperEngineOutput::Metric {
                    name: if is_final {
                        "whisper_final_flush_ms".to_string()
                    } else {
                        "whisper_rolling_inference_ms".to_string()
                    },
                    value: elapsed_ms,
                }];
                if !is_final && self.first_partial_emitted.insert(stream.to_string()) {
                    if let Some(first_audio_at) = self.first_audio_at.get(stream) {
                        outputs.push(WhisperEngineOutput::Metric {
                            name: "first_partial_latency_ms".to_string(),
                            value: first_audio_at.elapsed().as_secs_f64() * 1000.0,
                        });
                    }
                }

                outputs.push(WhisperEngineOutput::Transcript {
                    segment_id: format!("{stream}-{segment_suffix}"),
                    duration_ms: duration_ms(buffer.samples().len(), self.sample_rate),
                    start_ms: 0,
                    stream: stream.to_string(),
                    text: transcript.text,
                    is_final,
                    language: transcript.language,
                    confidence: transcript.confidence,
                });
                SidecarAction::outputs(outputs)
            }
            Ok(None) => SidecarAction::none(),
            Err(error) => SidecarAction::output(WhisperEngineOutput::Error {
                message: error.message,
                fatal: false,
            }),
        }
    }

    pub fn samples(&self, stream: &str) -> Option<&[i16]> {
        self.buffers.get(stream).map(PcmRingBuffer::samples)
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

fn duration_ms(sample_count: usize, sample_rate: u32) -> i64 {
    ((sample_count as f64 * 1000.0) / f64::from(sample_rate)).round() as i64
}

pub fn handle_engine_input(input: WhisperEngineInput) -> SidecarAction {
    match input {
        WhisperEngineInput::Config { .. } => SidecarAction::output(WhisperEngineOutput::Status {
            state: "ready".to_string(),
        }),
        WhisperEngineInput::Audio { .. } => SidecarAction::none(),
        WhisperEngineInput::Flush { .. } => SidecarAction::none(),
        WhisperEngineInput::Shutdown => SidecarAction::Shutdown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whisper_engine_protocol::{WhisperEngineInput, WhisperEngineOutput};
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    #[test]
    fn config_input_reports_ready_status() {
        let action = handle_engine_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        assert_eq!(
            action,
            SidecarAction::output(WhisperEngineOutput::Status {
                state: "ready".to_string()
            })
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
            SidecarAction::none()
        );
    }

    #[test]
    fn flush_input_continues_without_output_until_backend_is_connected() {
        assert_eq!(
            handle_engine_input(WhisperEngineInput::Flush {
                stream: "speaker".to_string()
            }),
            SidecarAction::none()
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

        assert_eq!(action, SidecarAction::none());
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
            SidecarAction::output(WhisperEngineOutput::Error {
                message: "invalid pcm16 audio payload".to_string(),
                fatal: false
            })
        );
    }

    #[derive(Default)]
    struct FakeBackend {
        loaded: Vec<crate::whisper_engine_backend::WhisperBackendConfig>,
        transcript: Option<crate::whisper_engine_backend::WhisperTranscription>,
        transcribed_samples: Vec<Vec<f32>>,
    }

    impl crate::whisper_engine_backend::WhisperBackend for FakeBackend {
        fn load_model(
            &mut self,
            config: crate::whisper_engine_backend::WhisperBackendConfig,
        ) -> Result<(), crate::whisper_engine_backend::WhisperBackendError> {
            self.loaded.push(config);
            Ok(())
        }

        fn transcribe(
            &mut self,
            _samples: &[f32],
        ) -> Result<
            Option<crate::whisper_engine_backend::WhisperTranscription>,
            crate::whisper_engine_backend::WhisperBackendError,
        > {
            self.transcribed_samples.push(_samples.to_vec());
            Ok(self.transcript.clone())
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
            SidecarAction::output(WhisperEngineOutput::Status {
                state: "ready".to_string()
            })
        );
        assert_eq!(sidecar.backend().loaded.len(), 1);
        assert_eq!(
            sidecar.backend().loaded[0].model_path,
            "/models/ggml-base.bin"
        );
    }

    struct FailingBackend;

    impl crate::whisper_engine_backend::WhisperBackend for FailingBackend {
        fn load_model(
            &mut self,
            _config: crate::whisper_engine_backend::WhisperBackendConfig,
        ) -> Result<(), crate::whisper_engine_backend::WhisperBackendError> {
            Err(crate::whisper_engine_backend::WhisperBackendError {
                message: "failed to load model".to_string(),
            })
        }

        fn transcribe(
            &mut self,
            _samples: &[f32],
        ) -> Result<
            Option<crate::whisper_engine_backend::WhisperTranscription>,
            crate::whisper_engine_backend::WhisperBackendError,
        > {
            Ok(None)
        }
    }

    #[test]
    fn config_input_reports_backend_load_failures_as_fatal_errors() {
        let mut sidecar = WhisperEngineSidecar::with_backend(4, FailingBackend);

        let action = sidecar.handle_input(WhisperEngineInput::Config {
            model_path: "/missing.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        assert_eq!(
            action,
            SidecarAction::output(WhisperEngineOutput::Error {
                message: "failed to load model".to_string(),
                fatal: true
            })
        );
    }

    #[test]
    fn flush_input_transcribes_buffered_stream_audio() {
        let backend = FakeBackend {
            transcript: Some(crate::whisper_engine_backend::WhisperTranscription {
                text: "hello".to_string(),
                language: "en".to_string(),
                confidence: Some(0.9),
            }),
            ..FakeBackend::default()
        };
        let mut sidecar = WhisperEngineSidecar::with_backend(16_000, backend);
        sidecar.handle_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });
        sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: "AIAAAAAA".to_string(),
        });

        let action = sidecar.handle_input(WhisperEngineInput::Flush {
            stream: "mic".to_string(),
        });

        assert_eq!(
            sidecar.backend().transcribed_samples,
            vec![vec![-1.0, 0.0, 0.0]]
        );
        let SidecarAction::Continue(outputs) = action else {
            panic!("expected output");
        };
        assert_metric_named(&outputs, "whisper_final_flush_ms");
        assert!(outputs.contains(&WhisperEngineOutput::Transcript {
            stream: "mic".to_string(),
            segment_id: "mic-flush".to_string(),
            text: "hello".to_string(),
            is_final: true,
            start_ms: 0,
            duration_ms: 0,
            language: "en".to_string(),
            confidence: Some(0.9)
        }));
    }

    #[test]
    fn audio_input_emits_interim_transcript_after_local_agreement() {
        let backend = FakeBackend {
            transcript: Some(crate::whisper_engine_backend::WhisperTranscription {
                text: "hel".to_string(),
                language: "en".to_string(),
                confidence: None,
            }),
            ..FakeBackend::default()
        };
        let mut sidecar = WhisperEngineSidecar::with_backend_and_step_samples(16_000, 2, backend);
        sidecar.handle_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        assert_eq!(
            sidecar.handle_input(WhisperEngineInput::Audio {
                stream: "mic".to_string(),
                seq: 1,
                timestamp_ms: 0,
                pcm16_base64: "AIAAAA==".to_string(),
            }),
            SidecarAction::none()
        );

        let action = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 2,
            timestamp_ms: 0,
            pcm16_base64: "AIAAAA==".to_string(),
        });

        let SidecarAction::Continue(outputs) = action else {
            panic!("expected output");
        };
        assert_metric_named(&outputs, "whisper_rolling_inference_ms");
        assert_metric_named(&outputs, "first_partial_latency_ms");
        assert!(outputs.contains(&WhisperEngineOutput::Transcript {
            stream: "mic".to_string(),
            segment_id: "mic-rolling".to_string(),
            text: "hel".to_string(),
            is_final: false,
            start_ms: 0,
            duration_ms: 0,
            language: "en".to_string(),
            confidence: None
        }));
    }

    #[test]
    fn audio_input_drops_backlogged_rolling_work_but_flush_still_transcribes() {
        let backend = FakeBackend {
            transcript: Some(crate::whisper_engine_backend::WhisperTranscription {
                text: "hello".to_string(),
                language: "en".to_string(),
                confidence: None,
            }),
            ..FakeBackend::default()
        };
        let mut sidecar = WhisperEngineSidecar::with_backend_and_step_samples(16_000, 2, backend);
        sidecar.handle_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        let action = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: STANDARD.encode([0u8; 16]),
        });

        assert_eq!(action, SidecarAction::none());
        assert!(sidecar.backend().transcribed_samples.is_empty());

        let action = sidecar.handle_input(WhisperEngineInput::Flush {
            stream: "mic".to_string(),
        });

        assert_eq!(sidecar.backend().transcribed_samples.len(), 1);
        let SidecarAction::Continue(outputs) = action else {
            panic!("expected output");
        };
        assert_metric_named(&outputs, "whisper_final_flush_ms");
        assert!(outputs.contains(&WhisperEngineOutput::Transcript {
            stream: "mic".to_string(),
            segment_id: "mic-flush".to_string(),
            text: "hello".to_string(),
            is_final: true,
            start_ms: 0,
            duration_ms: 1,
            language: "en".to_string(),
            confidence: None
        }));
    }

    fn assert_metric_named(outputs: &[WhisperEngineOutput], name: &str) {
        assert!(outputs.iter().any(|output| matches!(
            output,
            WhisperEngineOutput::Metric {
                name: metric_name,
                value
            } if metric_name == name && *value >= 0.0
        )));
    }
}
