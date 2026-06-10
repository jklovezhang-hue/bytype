//! 首启向导后端:就绪状态、依赖检测、模型下载/导入。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use voice_input::config::Config;

#[derive(Serialize)]
pub struct WizardState {
    pub ready: bool,
    pub config_exists: bool,
    pub model_present: bool,
    pub model_dir: String,
}

#[derive(Serialize)]
pub struct DepCheck {
    pub key: String,
    pub label: String,
    pub status: String, // "ok" | "bad" | "warn"
    pub detail: String,
    pub fix_url: Option<String>,
}

/// exe 所在目录。
pub(crate) fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// 向导阶段模型目录:有 config 用其解析后的 asr.model_dir,否则 exe 目录旁 models/sensevoice。
pub(crate) fn wizard_model_dir() -> PathBuf {
    if let Ok(cfg) = Config::load_resolved() {
        return PathBuf::from(cfg.asr.model_dir);
    }
    exe_dir()
        .map(|d| d.join("models").join("sensevoice"))
        .unwrap_or_else(|| PathBuf::from("models/sensevoice"))
}

/// 就绪状态:config 存在 + 模型齐全。供前端分流。
#[tauri::command]
pub fn wizard_state() -> WizardState {
    let config_exists = voice_input::config::find_config_file().is_ok();
    let dir = wizard_model_dir();
    let model_present = voice_input::readiness::model_present(&dir);
    WizardState {
        ready: config_exists && model_present,
        config_exists,
        model_present,
        model_dir: dir.display().to_string(),
    }
}

/// 4 项依赖检测。
#[tauri::command]
pub fn check_dependencies() -> Vec<DepCheck> {
    vec![vcredist_check(), core_dll_check(), mic_device_check(), mic_privacy_check()]
}

/// 用资源管理器打开 URL(支持 http(s) 与 ms-settings: 协议)。
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    // 仅放行 https 与 ms-settings(向导只会传这两类固定链接),防止任意路径被 explorer 当项打开。
    if !(url.starts_with("https://") || url.starts_with("ms-settings:")) {
        return Err(format!("拒绝打开非白名单 URL: {url}"));
    }
    std::process::Command::new("explorer")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- 依赖检测实现 ----

fn vcredist_check() -> DepCheck {
    let ok = vcredist_installed();
    DepCheck {
        key: "vcredist".into(),
        label: "VC++ 运行时".into(),
        status: if ok { "ok" } else { "bad" }.into(),
        detail: if ok {
            "Microsoft Visual C++ Redistributable 已安装".into()
        } else {
            "未检测到,onnxruntime 将无法加载".into()
        },
        fix_url: if ok { None } else { Some("https://aka.ms/vs/17/release/vc_redist.x64.exe".into()) },
    }
}

#[cfg(windows)]
fn vcredist_installed() -> bool {
    use windows::core::w;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    // 程序自身链接 MSVC 运行时,运行时该 dll 必已加载;GetModuleHandleW 查已加载模块,
    // 不增引用计数(LoadLibraryW 会泄漏一个引用)。
    unsafe {
        GetModuleHandleW(w!("vcruntime140.dll")).is_ok()
            || GetModuleHandleW(w!("vcruntime140_1.dll")).is_ok()
    }
}
#[cfg(not(windows))]
fn vcredist_installed() -> bool {
    true
}

fn core_dll_check() -> DepCheck {
    let ok = match exe_dir() {
        Some(d) => ["onnxruntime.dll", "sherpa-onnx-c-api.dll"].iter().all(|n| d.join(n).is_file()),
        None => false,
    };
    DepCheck {
        key: "core_dll".into(),
        label: "核心组件".into(),
        status: if ok { "ok" } else { "bad" }.into(),
        detail: if ok {
            "onnxruntime.dll / sherpa-onnx-c-api.dll 完整".into()
        } else {
            "缺少 onnxruntime.dll 或 sherpa-onnx-c-api.dll".into()
        },
        fix_url: None,
    }
}

fn mic_device_check() -> DepCheck {
    let ok = voice_input::audio::has_input_device();
    DepCheck {
        key: "mic_device".into(),
        label: "麦克风设备".into(),
        status: if ok { "ok" } else { "warn" }.into(),
        detail: if ok { "检测到可用录音设备".into() } else { "未检测到录音设备".into() },
        fix_url: None,
    }
}

fn mic_privacy_check() -> DepCheck {
    match mic_privacy_allowed() {
        Some(false) => DepCheck {
            key: "mic_privacy".into(),
            label: "麦克风权限".into(),
            status: "warn".into(),
            detail: "Windows 隐私设置可能禁止桌面应用访问麦克风".into(),
            fix_url: Some("ms-settings:privacy-microphone".into()),
        },
        // true 或 未知(None)都不报警
        _ => DepCheck {
            key: "mic_privacy".into(),
            label: "麦克风权限".into(),
            status: "ok".into(),
            detail: "已允许桌面应用访问麦克风".into(),
            fix_url: None,
        },
    }
}

/// 读注册表判断麦克风隐私权限;读不到返回 None(未知,不报警)。
#[cfg(windows)]
fn mic_privacy_allowed() -> Option<bool> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone")
        .ok()?;
    let val: String = key.get_value("Value").ok()?;
    Some(val == "Allow")
}
#[cfg(not(windows))]
fn mic_privacy_allowed() -> Option<bool> {
    None
}

/// 下载取消标志(由 lib.rs manage)。
#[derive(Default)]
pub struct DownloadCancel(pub Arc<AtomicBool>);

#[derive(Clone, Serialize)]
struct DlProgress {
    file: String,
    received: u64,
    total: u64,
}

/// 下载模型(tokens + model)到向导模型目录,emit `bt:dl-progress`。
#[tauri::command]
pub async fn download_model(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, DownloadCancel>,
) -> Result<(), String> {
    let flag = cancel.0.clone();
    flag.store(false, Ordering::SeqCst);
    let dir = wizard_model_dir();
    // 无 config 时用默认配置的 URL(仍是 hf-mirror 默认)。
    let cfg = Config::load_resolved().unwrap_or_default();
    let (model_url, tokens_url) = (cfg.model.model_url.clone(), cfg.model.tokens_url.clone());

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // 先 tokens(小、快,能早暴露 URL 错误),再 model(大),存为 model.onnx
        dl_one(&app, &flag, &tokens_url, &dir.join("tokens.txt"), "tokens")?;
        dl_one(&app, &flag, &model_url, &dir.join("model.onnx"), "model")?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn dl_one(
    app: &tauri::AppHandle,
    flag: &Arc<AtomicBool>,
    url: &str,
    dest: &Path,
    file_tag: &str,
) -> Result<(), String> {
    let part = dest.with_extension("part");
    let app2 = app.clone();
    let tag = file_tag.to_string();
    let res = voice_input::download::download_file(
        url,
        &part,
        |received, total| {
            let _ = app2.emit("bt:dl-progress", DlProgress { file: tag.clone(), received, total });
        },
        || flag.load(Ordering::SeqCst),
    );
    match res {
        Ok(()) => {
            // 校验大小:model ≥ 100MB,tokens ≥ 1KB
            let min: u64 = if file_tag == "model" { 100 * 1024 * 1024 } else { 1024 };
            let size = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
            if size < min {
                std::fs::remove_file(&part).ok();
                return Err(format!("{file_tag} 文件过小({size} 字节),可能下载不完整"));
            }
            std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            std::fs::remove_file(&part).ok();
            Err(e.to_string())
        }
    }
}

/// 取消进行中的下载。
#[tauri::command]
pub fn cancel_download(cancel: tauri::State<DownloadCancel>) {
    cancel.0.store(true, Ordering::SeqCst);
}

/// 导入用户本地已下好的模型文件(校验后复制到模型目录)。
#[tauri::command]
pub fn import_model(model_path: String, tokens_path: String) -> Result<(), String> {
    let dir = wizard_model_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let m = Path::new(&model_path);
    let t = Path::new(&tokens_path);
    let msize = std::fs::metadata(m).map(|x| x.len()).unwrap_or(0);
    if msize < 100 * 1024 * 1024 {
        return Err("所选模型文件过小,不像有效的 model.onnx".into());
    }
    if std::fs::metadata(t).map(|x| x.len()).unwrap_or(0) == 0 {
        return Err("所选 tokens 文件为空或不存在".into());
    }
    std::fs::copy(m, dir.join("model.onnx")).map_err(|e| e.to_string())?;
    std::fs::copy(t, dir.join("tokens.txt")).map_err(|e| e.to_string())?;
    Ok(())
}
