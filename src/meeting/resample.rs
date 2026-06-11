/// 线性插值重采样单声道 f32 到 16000Hz。`src_rate` 为输入采样率。
pub fn resample_to_16k(input: &[f32], src_rate: u32) -> Vec<f32> {
    const DST: u32 = 16000;
    if input.is_empty() {
        return Vec::new();
    }
    if src_rate == DST {
        return input.to_vec();
    }
    let ratio = src_rate as f64 / DST as f64;
    let out_len = ((input.len() as f64) / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::resample_to_16k;

    #[test]
    fn passthrough_when_already_16k() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let out = resample_to_16k(&input, 16000);
        assert_eq!(out, input);
    }

    #[test]
    fn halves_length_from_32k() {
        let input: Vec<f32> = (0..3200).map(|i| i as f32).collect();
        let out = resample_to_16k(&input, 32000);
        assert!((out.len() as i32 - 1600).abs() <= 1, "got {}", out.len());
    }

    #[test]
    fn upsamples_from_8k() {
        let input: Vec<f32> = (0..800).map(|i| i as f32).collect();
        let out = resample_to_16k(&input, 8000);
        assert!((out.len() as i32 - 1600).abs() <= 1, "got {}", out.len());
    }

    #[test]
    fn empty_input_yields_empty() {
        assert!(resample_to_16k(&[], 48000).is_empty());
    }
}
