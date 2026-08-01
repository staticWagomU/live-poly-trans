use base64::{engine::general_purpose::STANDARD, Engine as _};

#[derive(Debug)]
pub enum WhisperEngineAudioError {
    Base64(base64::DecodeError),
}

pub fn decode_pcm16_base64(encoded: &str) -> Result<Vec<i16>, WhisperEngineAudioError> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(WhisperEngineAudioError::Base64)?;

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
}
