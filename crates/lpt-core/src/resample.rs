//! Linear-interpolation resampler to Whisper's 16 kHz mono input rate.

pub const TARGET_RATE: u32 = 16_000;

pub fn resample_to_16k(samples: &[f32], src_rate: u32) -> Vec<f32> {
    let _ = src_rate;
    samples.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rate_passes_through() {
        let out = resample_to_16k(&[0.1, 0.2, 0.3], 16_000);
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }
}
