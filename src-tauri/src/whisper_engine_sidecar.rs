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
pub const DEFAULT_ROLLING_STEP_SAMPLES: usize = 16_000 * 500 / 1000;
pub const DEFAULT_MAX_ROLLING_BACKLOG_STEPS: usize = 2;
pub const DEFAULT_FINALIZE_SILENCE_SAMPLES: usize = 16_000 * 800 / 1000;

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
    latest_transcripts: HashMap<String, CachedTranscript>,
    trailing_silence_samples: HashMap<String, usize>,
    first_audio_at: HashMap<String, Instant>,
    first_partial_emitted: HashSet<String>,
    backend: B,
    sample_rate: u32,
    rolling_step_samples: usize,
    finalize_silence_samples: usize,
}

#[derive(Debug, Clone)]
struct CachedTranscript {
    text: String,
    language: String,
    confidence: Option<f64>,
    start_ms: i64,
    duration_ms: i64,
    sample_count: usize,
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
            latest_transcripts: HashMap::new(),
            trailing_silence_samples: HashMap::new(),
            first_audio_at: HashMap::new(),
            first_partial_emitted: HashSet::new(),
            backend: NoopWhisperBackend::default(),
            sample_rate: 16_000,
            rolling_step_samples: DEFAULT_ROLLING_STEP_SAMPLES,
            finalize_silence_samples: DEFAULT_FINALIZE_SILENCE_SAMPLES,
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
        Self::with_backend_step_and_silence_samples(
            capacity_samples,
            rolling_step_samples,
            DEFAULT_FINALIZE_SILENCE_SAMPLES,
            backend,
        )
    }

    fn with_backend_step_and_silence_samples(
        capacity_samples: usize,
        rolling_step_samples: usize,
        finalize_silence_samples: usize,
        backend: B,
    ) -> Self {
        Self {
            capacity_samples,
            buffers: HashMap::new(),
            schedulers: HashMap::new(),
            stabilizers: HashMap::new(),
            latest_transcripts: HashMap::new(),
            trailing_silence_samples: HashMap::new(),
            first_audio_at: HashMap::new(),
            first_partial_emitted: HashSet::new(),
            backend,
            sample_rate: 16_000,
            rolling_step_samples,
            finalize_silence_samples,
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

                let is_silence = is_silent_frame(&stream, &samples);
                if is_silence && !self.buffers.contains_key(&stream) {
                    return SidecarAction::none();
                }

                self.first_audio_at
                    .entry(stream.clone())
                    .or_insert_with(Instant::now);
                self.buffers
                    .entry(stream.clone())
                    .or_insert_with(|| PcmRingBuffer::new(self.capacity_samples))
                    .push(&samples);
                if is_silence {
                    *self
                        .trailing_silence_samples
                        .entry(stream.clone())
                        .or_default() += samples.len();
                } else {
                    self.trailing_silence_samples.remove(&stream);
                }

                let Some(buffer) = self.buffers.get(&stream) else {
                    return SidecarAction::none();
                };
                let scheduler = self.schedulers.entry(stream.clone()).or_insert_with(|| {
                    RollingTranscriptionScheduler::new(self.rolling_step_samples)
                });
                let mut outputs = Vec::new();
                match scheduler.observe_total_samples_with_backlog_limit(
                    buffer.samples().len(),
                    DEFAULT_MAX_ROLLING_BACKLOG_STEPS,
                ) {
                    RollingTranscriptionDecision::Wait
                    | RollingTranscriptionDecision::DropBacklog => {}
                    RollingTranscriptionDecision::Transcribe => {
                        outputs.extend(outputs_from_action(
                            self.transcribe_buffer(&stream, false, "rolling"),
                        ));
                    }
                }

                if self
                    .trailing_silence_samples
                    .get(&stream)
                    .is_some_and(|samples| *samples >= self.finalize_silence_samples)
                {
                    outputs.extend(outputs_from_action(self.finalize_after_silence(&stream)));
                }

                return SidecarAction::outputs(outputs);
            }
            WhisperEngineInput::Flush { stream } => {
                let action = self.flush_stream(&stream);
                if let Some(stabilizer) = self.stabilizers.get_mut(&stream) {
                    stabilizer.reset();
                }
                self.first_audio_at.remove(&stream);
                self.first_partial_emitted.remove(&stream);
                self.trailing_silence_samples.remove(&stream);
                return action;
            }
            input => handle_engine_input(input),
        }
    }

    fn finalize_after_silence(&mut self, stream: &str) -> SidecarAction {
        let Some(cached) = self.latest_transcripts.get(stream) else {
            return SidecarAction::none();
        };
        if !self.cached_transcript_covers_speech(stream, cached) {
            return SidecarAction::none();
        }

        let action = self.promote_cached_transcript(stream);
        if let Some(stabilizer) = self.stabilizers.get_mut(stream) {
            stabilizer.reset();
        }
        self.first_audio_at.remove(stream);
        self.first_partial_emitted.remove(stream);
        self.trailing_silence_samples.remove(stream);
        action
    }

    fn flush_stream(&mut self, stream: &str) -> SidecarAction {
        if let Some(cached) = self.latest_transcripts.get(stream) {
            if !self.cached_transcript_covers_speech(stream, cached) {
                let action = self.transcribe_buffer(stream, true, "flush");
                self.latest_transcripts.remove(stream);
                self.buffers.remove(stream);
                self.schedulers.remove(stream);
                return action;
            }
        }

        self.promote_cached_transcript(stream)
    }

    fn cached_transcript_covers_speech(&self, stream: &str, cached: &CachedTranscript) -> bool {
        let current_sample_count = self
            .buffers
            .get(stream)
            .map(|buffer| buffer.samples().len())
            .unwrap_or_default();
        if current_sample_count <= cached.sample_count {
            return true;
        }

        let uncovered_samples = current_sample_count - cached.sample_count;
        let trailing_silence_samples = self
            .trailing_silence_samples
            .get(stream)
            .copied()
            .unwrap_or_default();
        uncovered_samples <= trailing_silence_samples
    }

    fn promote_cached_transcript(&mut self, stream: &str) -> SidecarAction {
        if let Some(cached) = self.latest_transcripts.remove(stream) {
            self.buffers.remove(stream);
            self.schedulers.remove(stream);
            return SidecarAction::outputs(vec![
                WhisperEngineOutput::Metric {
                    name: "whisper_final_flush_ms".to_string(),
                    value: 0.0,
                },
                WhisperEngineOutput::Transcript {
                    segment_id: format!("{stream}-flush"),
                    duration_ms: cached.duration_ms,
                    start_ms: cached.start_ms,
                    stream: stream.to_string(),
                    text: cached.text,
                    is_final: true,
                    language: cached.language,
                    confidence: cached.confidence,
                },
            ]);
        }

        let action = self.transcribe_buffer(stream, true, "flush");
        self.buffers.remove(stream);
        self.schedulers.remove(stream);
        self.latest_transcripts.remove(stream);
        action
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
                    let Some(stable_text) =
                        stabilizer.observe_with_initial_provisional(&transcript.text)
                    else {
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

                let duration_ms = duration_ms(buffer.samples().len(), self.sample_rate);
                if !is_final {
                    self.latest_transcripts.insert(
                        stream.to_string(),
                        CachedTranscript {
                            text: transcript.text.clone(),
                            language: transcript.language.clone(),
                            confidence: transcript.confidence,
                            start_ms: 0,
                            duration_ms,
                            sample_count: buffer.samples().len(),
                        },
                    );
                }

                outputs.push(WhisperEngineOutput::Transcript {
                    segment_id: format!("{stream}-{segment_suffix}"),
                    duration_ms,
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

fn outputs_from_action(action: SidecarAction) -> Vec<WhisperEngineOutput> {
    match action {
        SidecarAction::Continue(outputs) => outputs,
        SidecarAction::Shutdown => Vec::new(),
    }
}

fn is_silent_frame(stream: &str, samples: &[i16]) -> bool {
    let threshold = match stream {
        "mic" => 0.01,
        "speaker" => 0.001,
        _ => 0.001,
    };
    pcm16_rms(samples) <= threshold
}

fn pcm16_rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_squares = samples
        .iter()
        .map(|sample| {
            let normalized = f64::from(*sample) / 32768.0;
            normalized * normalized
        })
        .sum::<f64>();
    (sum_squares / samples.len() as f64).sqrt()
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
            pcm16_base64: STANDARD.encode([0, 128, 255, 127]),
        });

        assert_eq!(action, SidecarAction::none());
        assert_eq!(sidecar.samples("mic"), Some(&[-32768, 32767][..]));
    }

    #[test]
    fn silent_audio_without_active_stream_is_ignored() {
        let mut sidecar = WhisperEngineSidecar::new(4);

        let action = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: STANDARD.encode([0u8; 4]),
        });

        assert_eq!(action, SidecarAction::none());
        assert_eq!(sidecar.samples("mic"), None);
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
        transcript_sequence: Vec<Option<crate::whisper_engine_backend::WhisperTranscription>>,
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
            if !self.transcript_sequence.is_empty() {
                return Ok(self.transcript_sequence.remove(0));
            }
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
    fn audio_input_emits_initial_provisional_then_stable_interim() {
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

        let first = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: "AIAAAA==".to_string(),
        });

        let SidecarAction::Continue(first_outputs) = first else {
            panic!("expected first output");
        };
        assert_metric_named(&first_outputs, "first_partial_latency_ms");
        assert!(first_outputs.contains(&WhisperEngineOutput::Transcript {
            stream: "mic".to_string(),
            segment_id: "mic-rolling".to_string(),
            text: "hel".to_string(),
            is_final: false,
            start_ms: 0,
            duration_ms: 0,
            language: "en".to_string(),
            confidence: None
        }));

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
    fn flush_promotes_latest_rolling_transcript_without_retranscribing() {
        let backend = FakeBackend {
            transcript: Some(crate::whisper_engine_backend::WhisperTranscription {
                text: "hello".to_string(),
                language: "en".to_string(),
                confidence: Some(0.8),
            }),
            ..FakeBackend::default()
        };
        let mut sidecar = WhisperEngineSidecar::with_backend_and_step_samples(16_000, 2, backend);
        sidecar.handle_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        let rolling = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: "AIAAAA==".to_string(),
        });
        let SidecarAction::Continue(rolling_outputs) = rolling else {
            panic!("expected rolling output");
        };
        assert!(rolling_outputs.contains(&WhisperEngineOutput::Transcript {
            stream: "mic".to_string(),
            segment_id: "mic-rolling".to_string(),
            text: "hello".to_string(),
            is_final: false,
            start_ms: 0,
            duration_ms: 0,
            language: "en".to_string(),
            confidence: Some(0.8)
        }));
        assert_eq!(sidecar.backend().transcribed_samples.len(), 1);

        let flush = sidecar.handle_input(WhisperEngineInput::Flush {
            stream: "mic".to_string(),
        });

        assert_eq!(sidecar.backend().transcribed_samples.len(), 1);
        let SidecarAction::Continue(outputs) = flush else {
            panic!("expected flush output");
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
            confidence: Some(0.8)
        }));
    }

    #[test]
    fn flush_retranscribes_when_buffer_has_audio_after_latest_rolling_result() {
        let backend = FakeBackend {
            transcript_sequence: vec![
                Some(crate::whisper_engine_backend::WhisperTranscription {
                    text: "hello".to_string(),
                    language: "en".to_string(),
                    confidence: Some(0.8),
                }),
                Some(crate::whisper_engine_backend::WhisperTranscription {
                    text: "hello tail".to_string(),
                    language: "en".to_string(),
                    confidence: Some(0.9),
                }),
            ],
            ..FakeBackend::default()
        };
        let mut sidecar = WhisperEngineSidecar::with_backend_and_step_samples(16_000, 2, backend);
        sidecar.handle_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        let rolling = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: "AIAAAA==".to_string(),
        });
        let SidecarAction::Continue(_) = rolling else {
            panic!("expected rolling output");
        };

        let wait = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 2,
            timestamp_ms: 0,
            pcm16_base64: "AIA=".to_string(),
        });
        assert_eq!(wait, SidecarAction::none());

        let flush = sidecar.handle_input(WhisperEngineInput::Flush {
            stream: "mic".to_string(),
        });

        assert_eq!(sidecar.backend().transcribed_samples.len(), 2);
        assert_eq!(sidecar.backend().transcribed_samples[1].len(), 3);
        let SidecarAction::Continue(outputs) = flush else {
            panic!("expected flush output");
        };
        assert_metric_named(&outputs, "whisper_final_flush_ms");
        assert!(outputs.contains(&WhisperEngineOutput::Transcript {
            stream: "mic".to_string(),
            segment_id: "mic-flush".to_string(),
            text: "hello tail".to_string(),
            is_final: true,
            start_ms: 0,
            duration_ms: 0,
            language: "en".to_string(),
            confidence: Some(0.9)
        }));
    }

    #[test]
    fn trailing_silence_finalizes_the_latest_speech_covering_rolling_transcript() {
        let backend = FakeBackend {
            transcript_sequence: vec![
                Some(crate::whisper_engine_backend::WhisperTranscription {
                    text: "hello".to_string(),
                    language: "en".to_string(),
                    confidence: Some(0.8),
                }),
                None,
            ],
            ..FakeBackend::default()
        };
        let mut sidecar =
            WhisperEngineSidecar::with_backend_step_and_silence_samples(16_000, 2, 2, backend);
        sidecar.handle_input(WhisperEngineInput::Config {
            model_path: "/models/ggml-base.bin".to_string(),
            language: "auto".to_string(),
            sample_rate: 16_000,
        });

        let rolling = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 1,
            timestamp_ms: 0,
            pcm16_base64: "AIAAAA==".to_string(),
        });
        let SidecarAction::Continue(rolling_outputs) = rolling else {
            panic!("expected rolling output");
        };
        assert!(rolling_outputs.iter().any(|output| matches!(
            output,
            WhisperEngineOutput::Transcript {
                is_final: false,
                text,
                ..
            } if text == "hello"
        )));

        let finalizing_silence = sidecar.handle_input(WhisperEngineInput::Audio {
            stream: "mic".to_string(),
            seq: 2,
            timestamp_ms: 0,
            pcm16_base64: STANDARD.encode([0u8; 4]),
        });

        assert_eq!(sidecar.backend().transcribed_samples.len(), 2);
        let SidecarAction::Continue(outputs) = finalizing_silence else {
            panic!("expected final output");
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
            confidence: Some(0.8)
        }));

        assert_eq!(
            sidecar.handle_input(WhisperEngineInput::Flush {
                stream: "mic".to_string()
            }),
            SidecarAction::none()
        );
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
            pcm16_base64: STANDARD.encode([
                0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128,
            ]),
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
