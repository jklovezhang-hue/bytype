//! 会议页后端:列历史会议、读单场、重新生成纪要、删除、打开文件夹。
use std::path::{Path, PathBuf};
use serde::Serialize;
use voice_input::config::Config;

/// 会议根目录(load_resolved 已把 output_dir 绝对化)。
fn meetings_root() -> PathBuf {
    match Config::load_resolved() {
        Ok(c) => PathBuf::from(c.meeting.output_dir),
        Err(_) => PathBuf::from("./meetings"),
    }
}

/// 校验 base 是单一安全目录名,防路径穿越(`base` 来自 webview)。
/// 要求:非空、不含路径分隔符、无 `..`、恰好一个路径成分。
fn ensure_safe_base(base: &str) -> Result<(), String> {
    let b = base.trim();
    let ok = !b.is_empty()
        && !b.contains('/')
        && !b.contains('\\')
        && !b.contains("..")
        && Path::new(b).components().count() == 1;
    if ok {
        Ok(())
    } else {
        Err("非法的会议标识".to_string())
    }
}

#[derive(Serialize)]
pub struct MeetingSummary {
    pub base: String,
    pub has_md: bool,
    pub has_mp3: bool,
}

#[tauri::command]
pub fn list_meetings() -> Vec<MeetingSummary> {
    let root = meetings_root();
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                let base = e.file_name().to_string_lossy().to_string();
                let dir = e.path();
                out.push(MeetingSummary {
                    has_md: dir.join(format!("{base}.md")).exists(),
                    has_mp3: dir.join(format!("{base}.mp3")).exists(),
                    base,
                });
            }
        }
    }
    out.sort_by(|a, b| b.base.cmp(&a.base));
    out
}

#[derive(Serialize)]
pub struct MeetingDetail {
    pub base: String,
    pub md: String,
    pub has_json: bool,
    pub has_mp3: bool,
}

#[tauri::command]
pub fn get_meeting(base: String) -> MeetingDetail {
    if ensure_safe_base(&base).is_err() {
        return MeetingDetail { has_json: false, has_mp3: false, base, md: String::new() };
    }
    let dir = meetings_root().join(&base);
    let md = std::fs::read_to_string(dir.join(format!("{base}.md"))).unwrap_or_default();
    MeetingDetail {
        has_json: dir.join(format!("{base}.json")).exists(),
        has_mp3: dir.join(format!("{base}.mp3")).exists(),
        base,
        md,
    }
}

#[tauri::command]
pub fn regenerate_minutes(base: String) -> Result<String, String> {
    ensure_safe_base(&base)?;
    let dir = meetings_root().join(&base);
    let json = std::fs::read_to_string(dir.join(format!("{base}.json")))
        .map_err(|_| "找不到转写数据(.json),无法重新生成".to_string())?;
    let t = voice_input::meeting::Transcript::from_json(&json)
        .map_err(|e| format!("解析转写失败:{e}"))?;
    let cfg = Config::load_resolved().map_err(|e| format!("加载配置失败:{e}"))?;
    let input = voice_input::meeting::transcript_to_input(&t);
    let minutes = if cfg.llm.enabled && !cfg.llm.base_url.trim().is_empty() {
        match voice_input::corrector::generate_minutes(
            &cfg.llm,
            &cfg.meeting.effective_minutes_prompt(),
            &input,
        ) {
            Ok(m) => Some(m),
            Err(e) => return Err(format!("纪要生成失败:{e}")),
        }
    } else {
        return Err("未启用/配置 LLM,无法生成纪要".into());
    };
    let md = voice_input::meeting::assemble_md(&base, minutes.as_deref(), &t);
    std::fs::write(dir.join(format!("{base}.md")), &md).map_err(|e| format!("写 md 失败:{e}"))?;
    Ok(md)
}

#[tauri::command]
pub fn delete_meeting(base: String) -> Result<(), String> {
    ensure_safe_base(&base)?;
    let dir = meetings_root().join(&base);
    std::fs::remove_dir_all(&dir).map_err(|e| format!("删除失败:{e}"))
}

#[tauri::command]
pub fn open_meeting_folder(base: String) -> Result<(), String> {
    ensure_safe_base(&base)?;
    let root = meetings_root();
    let dir = root.join(&base);
    // 该场会议目录不存在时,退回打开会议根目录(避免给 explorer 传不存在的路径 → 它会转而弹出"文档")。
    let target = if dir.is_dir() { dir } else { root };
    std::process::Command::new("explorer")
        .arg(target)
        .spawn()
        .map_err(|e| format!("打开失败:{e}"))?;
    Ok(())
}
