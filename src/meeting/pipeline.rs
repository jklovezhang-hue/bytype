use std::path::Path;
use anyhow::{Context, Result};

use crate::corrector::Corrector;
use super::diarize::diarize_segments;
use super::segment::{vad_segments, Segment};
use super::transcribe::SegmentTranscriber;
use super::transcript::{Line, Speaker, Transcript};

/// 读 16k 单声道 WAV 为 f32 样本(不存在则返回空)。
fn read_wav_mono(path: &Path) -> Result<Vec<f32>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut r = hound::WavReader::open(path)
        .with_context(|| format!("读 WAV 失败: {}", path.display()))?;
    let samples: Vec<f32> = r
        .samples::<i16>()
        .filter_map(|s| s.ok())
        .map(|s| s as f32 / 32768.0)
        .collect();
    Ok(samples)
}

/// 把一条轨切段 + 识别 + 打说话人标签,产出行列表。
fn lines_for_track(
    samples: &[f32],
    speaker: Speaker,
    vad_model: &str,
    tr: &mut SegmentTranscriber,
) -> Result<Vec<Line>> {
    let segs: Vec<Segment> = vad_segments(samples, vad_model)?;
    let mut lines = Vec::new();
    for seg in segs {
        let text = tr.transcribe_seg(&seg.samples);
        if text.trim().is_empty() {
            continue;
        }
        lines.push(Line {
            start_ms: seg.start_ms(),
            end_ms: seg.end_ms(),
            speaker: speaker.clone(),
            text,
        });
    }
    Ok(lines)
}

/// 对转写逐行做 LLM 清理(就地修改 text)。
pub fn clean_transcript(t: &mut Transcript, corrector: &Corrector) {
    for l in &mut t.lines {
        l.text = corrector.clean_line(&l.text);
    }
}

/// diarization 选项。
pub struct DiarOpts<'a> {
    pub enabled: bool,
    pub segmentation_model: &'a str,
    pub embedding_model: &'a str,
    pub speakers: i32,
}

/// 离线转写一场会议:mic→「我」,system→「对方」,按时间归并。
pub fn transcribe_meeting(
    base: &str,
    mic_wav: &Path,
    system_wav: &Path,
    asr_model_dir: &str,
    language: &str,
    vad_model: &str,
    diar: DiarOpts,
) -> Result<Transcript> {
    let mut tr = SegmentTranscriber::load(asr_model_dir, language)?;
    let mic = read_wav_mono(mic_wav)?;
    let sys = read_wav_mono(system_wav)?;
    let mine = lines_for_track(&mic, Speaker::Me, vad_model, &mut tr)?;

    let theirs = if diar.enabled
        && !sys.is_empty()
        && Path::new(diar.segmentation_model).exists()
        && Path::new(diar.embedding_model).exists()
    {
        match lines_for_system_diarized(&sys, &diar, &mut tr) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("说话人分离失败,退化为不分人:{e}");
                lines_for_track(&sys, Speaker::Other, vad_model, &mut tr)?
            }
        }
    } else {
        lines_for_track(&sys, Speaker::Other, vad_model, &mut tr)?
    };
    Ok(Transcript::merge(base, mine, theirs))
}

fn lines_for_system_diarized(
    samples: &[f32],
    diar: &DiarOpts,
    tr: &mut SegmentTranscriber,
) -> Result<Vec<Line>> {
    let segs = diarize_segments(samples, diar.segmentation_model, diar.embedding_model, diar.speakers)?;
    let mut lines = Vec::new();
    let n = samples.len();
    for s in segs {
        let a = ((s.start_ms * 16) as usize).min(n);
        let b = ((s.end_ms * 16) as usize).min(n);
        if b <= a {
            continue;
        }
        let text = tr.transcribe_seg(&samples[a..b]);
        if text.trim().is_empty() {
            continue;
        }
        lines.push(Line {
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            speaker: Speaker::OtherId(s.speaker as u32 + 1),
            text,
        });
    }
    Ok(lines)
}
