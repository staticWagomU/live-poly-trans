use base64::{engine::general_purpose::STANDARD, Engine as _};

#[derive(Debug)]
pub enum WhisperEngineAudioError {
    Base64(base64::DecodeError),
    OddByteLength { byte_len: usize },
}

pub fn decode_pcm16_base64(encoded: &str) -> Result<Vec<i16>, WhisperEngineAudioError> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(WhisperEngineAudioError::Base64)?;
    if bytes.len() % 2 != 0 {
        return Err(WhisperEngineAudioError::OddByteLength {
            byte_len: bytes.len(),
        });
    }

    Ok(bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect())
}

pub fn pcm16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples
        .iter()
        .map(|sample| f32::from(*sample) / 32768.0)
        .collect()
}

pub struct PcmRingBuffer {
    capacity_samples: usize,
    samples: Vec<i16>,
}

impl PcmRingBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            capacity_samples,
            samples: Vec::new(),
        }
    }

    pub fn push(&mut self, samples: &[i16]) {
        self.samples.extend_from_slice(samples);
        if self.samples.len() > self.capacity_samples {
            self.samples
                .drain(0..self.samples.len() - self.capacity_samples);
        }
    }

    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_little_endian_pcm16_base64_samples() {
        let samples = decode_pcm16_base64("AQD//w==").unwrap();

        assert_eq!(samples, vec![1, -1]);
    }

    #[test]
    fn rejects_odd_byte_length_pcm16_payloads() {
        let result = decode_pcm16_base64("AA==");

        assert!(matches!(
            result,
            Err(WhisperEngineAudioError::OddByteLength { byte_len: 1 })
        ));
    }

    #[test]
    fn ring_buffer_keeps_only_the_newest_samples() {
        let mut buffer = PcmRingBuffer::new(4);

        buffer.push(&[1, 2, 3]);
        buffer.push(&[4, 5, 6]);

        assert_eq!(buffer.samples(), &[3, 4, 5, 6]);
    }

    #[test]
    fn converts_pcm16_samples_to_whisper_float_samples() {
        let samples = pcm16_to_f32(&[-32768, 0, 32767]);

        assert_eq!(samples, vec![-1.0, 0.0, 32767.0 / 32768.0]);
    }
}
