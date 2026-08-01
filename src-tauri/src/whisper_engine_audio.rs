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
}
