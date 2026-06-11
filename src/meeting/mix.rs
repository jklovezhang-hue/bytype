/// 把若干条 i16 单声道轨混音成一条:逐采样相加(i32 累加防中途溢出),
/// 若峰值超出 i16 范围则整体线性缩放(防削波),最后量化回 i16。短轨按最长补零。
pub fn mix_tracks(tracks: &[Vec<i16>]) -> Vec<i16> {
    let len = tracks.iter().map(|t| t.len()).max().unwrap_or(0);
    if len == 0 {
        return Vec::new();
    }
    let mut acc = vec![0i32; len];
    for t in tracks {
        for (i, &s) in t.iter().enumerate() {
            acc[i] += s as i32;
        }
    }
    let peak = acc.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
    let limit = i16::MAX as u32; // 32767
    let scale = if peak > limit { limit as f64 / peak as f64 } else { 1.0 };
    acc.iter()
        .map(|&v| {
            let scaled = (v as f64 * scale).round();
            scaled.clamp(i16::MIN as f64, i16::MAX as f64) as i16
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::mix_tracks;

    #[test]
    fn single_track_passthrough() {
        let a = vec![100i16, -200, 300];
        assert_eq!(mix_tracks(&[a.clone()]), a);
    }

    #[test]
    fn sums_two_tracks_without_clipping() {
        let a = vec![1000i16, 2000, -1000];
        let b = vec![500i16, -500, 1000];
        assert_eq!(mix_tracks(&[a, b]), vec![1500i16, 1500, 0]);
    }

    #[test]
    fn pads_shorter_track_with_zeros() {
        let a = vec![100i16, 100, 100];
        let b = vec![50i16];
        assert_eq!(mix_tracks(&[a, b]), vec![150i16, 100, 100]);
    }

    #[test]
    fn scales_down_to_prevent_clipping() {
        let a = vec![30000i16];
        let b = vec![20000i16];
        let out = mix_tracks(&[a, b]);
        assert_eq!(out.len(), 1);
        // 30000+20000=50000 超 i16,缩放后应贴近满幅(i16::MAX 上限恒成立,只断言下界)。
        assert!(out[0] >= 32000, "got {}", out[0]);
    }

    #[test]
    fn empty_yields_empty() {
        let empty: Vec<Vec<i16>> = vec![];
        assert!(mix_tracks(&empty).is_empty());
    }
}
