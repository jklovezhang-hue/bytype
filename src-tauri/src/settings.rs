//! 设置界面后端:读写 config.toml、LLM 连通测试、重启应用、打开配置目录。

use std::path::PathBuf;

use serde::Serialize;
use voice_input::config::{find_config_file, Config, LlmConfig};

#[derive(Serialize)]
pub struct GetConfigResp {
    pub config: Config,
    /// config.toml 的实际路径;找不到文件时为 None。
    pub path: Option<String>,
    /// 文件存在但解析失败时的错误信息(此时 config 为默认值)。
    pub error: Option<String>,
}

/// 读取**原始**配置(路径字段不解析,保证相对路径原样往返)。
/// (不直接用 Config::load_raw:解析失败时它丢失文件路径,而 UI 黄条需要"路径+错误"同时返回。)
#[tauri::command]
pub fn get_config() -> GetConfigResp {
    match find_config_file() {
        Ok((file, _dir)) => {
            let path = Some(file.display().to_string());
            match Config::load(&file.to_string_lossy()) {
                Ok(config) => GetConfigResp { config, path, error: None },
                Err(e) => GetConfigResp {
                    config: Config::default(),
                    path,
                    error: Some(format!("{e:#}")),
                },
            }
        }
        Err(_) => GetConfigResp { config: Config::default(), path: None, error: None },
    }
}

/// 整文件写回 config.toml;找不到原文件时写到程序目录。
/// (与 get_config 各自独立查找路径:设置窗口打开期间用户手动移走 config.toml 时,
/// 保存会回落到程序目录新建——内容不丢,属已知可接受行为。)
#[tauri::command]
pub fn save_config(config: Config) -> Result<(), String> {
    let path = match find_config_file() {
        Ok((file, _)) => file,
        Err(_) => exe_dir().ok_or("无法确定程序目录")?.join("config.toml"),
    };
    config.save_to(&path).map_err(|e| format!("{e:#}"))
}

#[derive(Serialize)]
pub struct TestOk {
    pub latency_ms: u64,
    pub reply: String,
}

/// 用表单当前的 [llm] 值测试连通(阻塞 HTTP 放 spawn_blocking,不卡 UI 线程)。
#[tauri::command]
pub async fn test_llm(llm: LlmConfig) -> Result<TestOk, String> {
    tauri::async_runtime::spawn_blocking(move || {
        voice_input::corrector::test_connection(&llm)
            .map(|(latency_ms, reply)| TestOk { latency_ms, reply })
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 保存成功后由前端调用:重启应用,使新配置生效。
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

/// 用资源管理器打开 config.toml 所在目录(找不到则打开程序目录)。
#[tauri::command]
pub fn open_config_dir() -> Result<(), String> {
    let dir = match find_config_file() {
        Ok((_, dir)) => dir,
        Err(_) => exe_dir().ok_or("无法确定程序目录")?,
    };
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}
