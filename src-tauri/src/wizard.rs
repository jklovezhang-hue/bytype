//! 首启向导后端:就绪状态、依赖检测、模型下载/导入。

use std::path::PathBuf;

use serde::Serialize;
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
