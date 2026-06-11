use anyhow::{anyhow, Result};
use sherpa_rs::diarize::{Diarize, DiarizeConfig};

/// 一段说话人分离结果:起止毫秒 + 说话人编号(0 起)。
#[derive(Debug, Clone)]
pub struct DiarSeg {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: i32,
}

/// 对 16k 单声道样本做说话人分离。`speakers`>0 固定人数,否则自动(阈值聚类)。
pub fn diarize_segments(
    samples: &[f32],
    segmentation_model: &str,
    embedding_model: &str,
    speakers: i32,
) -> Result<Vec<DiarSeg>> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let config = DiarizeConfig {
        num_clusters: Some(if speakers > 0 { speakers } else { -1 }),
        threshold: Some(0.5),
        min_duration_on: Some(0.3),
        min_duration_off: Some(0.5),
        provider: None,
        debug: false,
    };
    let mut d = Diarize::new(segmentation_model, embedding_model, config)
        .map_err(|e| anyhow!("创建说话人分离失败(确认模型存在): {e}"))?;
    let segs = d
        .compute(samples.to_vec(), None)
        .map_err(|e| anyhow!("说话人分离计算失败: {e}"))?;
    Ok(segs
        .into_iter()
        .map(|s| DiarSeg {
            start_ms: (s.start * 1000.0) as u64,
            end_ms: (s.end * 1000.0) as u64,
            speaker: s.speaker,
        })
        .collect())
}
