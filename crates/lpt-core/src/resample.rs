//! Linear-interpolation resampler to Whisper's 16 kHz mono input rate.

pub const TARGET_RATE: u32 = 16_000;

pub fn resample_to_16k(samples: &[f32], src_rate: u32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let ratio = src_rate as f64 / TARGET_RATE as f64;
    let out_len = (samples.len() as f64 / ratio).floor() as usize;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;
            let a = samples[idx];
            let b = samples[(idx + 1).min(samples.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rate_passes_through() {
        let out = resample_to_16k(&[0.1, 0.2, 0.3], 16_000);
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn downsamples_48k_by_taking_every_third_position() {
        let input = [0.0, 3.0, 6.0, 9.0, 12.0, 15.0];
        let out = resample_to_16k(&input, 48_000);
        assert_eq!(out, vec![0.0, 9.0]);
    }
}
