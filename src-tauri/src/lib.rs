mod settings;
mod wizard;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent,
};

use voice_input::config::Config;
use voice_input::engine::{bottom_center, ControlHandle, EngineObserver, OverlayState};

/// 浮窗逻辑像素尺寸(须与 tauri.conf.json 的 overlay 窗口一致)。
const OVERLAY_W: i32 = 240;
const OVERLAY_H: i32 = 64;
/// 距屏幕底部的逻辑像素留白(避开任务栏)。
const OVERLAY_BOTTOM_MARGIN: f64 = 80.0;

/// 存放引擎交回的取消句柄,供 cancel_recording 命令使用。
#[derive(Default)]
struct ControlSlot(Mutex<Option<ControlHandle>>);

/// 引擎是否已启动(防止 setup 与 finish_wizard 重复启动钩子/录音器)。
#[derive(Default)]
struct EngineStarted(AtomicBool);

/// 前端点药丸时调用:请求取消当前录音(跳过 LLM)。
#[tauri::command]
fn cancel_recording(slot: State<ControlSlot>) {
    if let Some(c) = slot.0.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        c.cancel();
    }
}

/// 驱动浮窗的观察者:状态变化时定位/显示浮窗并向前端 emit。
struct TauriObserver {
    app: tauri::AppHandle,
    enabled: bool,
}

impl EngineObserver for TauriObserver {
    fn on_ready(&self, control: ControlHandle) {
        self.app
            .state::<ControlSlot>()
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .replace(control);
    }

    fn on_state(&self, state: OverlayState) {
        if !self.enabled {
            return;
        }
        let tag = match state {
            OverlayState::Recording => "recording",
            OverlayState::Processing => "processing",
            OverlayState::Done => "done",
            OverlayState::Cancelled => "cancelled",
            OverlayState::Failed => "failed",
        };
        if matches!(state, OverlayState::Recording) {
            if let Some(w) = self.app.get_webview_window("overlay") {
                position_bottom_center(&w);
                let _ = w.show();
            }
        }
        let _ = self.app.emit_to("overlay", "bt:state", tag);
    }
}

/// 把浮窗摆到主屏底部正中(物理像素)。
fn position_bottom_center(w: &WebviewWindow) {
    if let Ok(Some(m)) = w.primary_monitor() {
        let scale = m.scale_factor();
        let ms = m.size();
        let win_w = (OVERLAY_W as f64 * scale) as i32;
        let win_h = (OVERLAY_H as f64 * scale) as i32;
        let margin = (OVERLAY_BOTTOM_MARGIN * scale) as i32;
        let (x, y) = bottom_center(ms.width as i32, ms.height as i32, win_w, win_h, margin);
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

/// 给浮窗 HWND 加 WS_EX_NOACTIVATE(点击/显示都不抢焦点)+ WS_EX_TOOLWINDOW(隐藏出 Alt-Tab)。
#[cfg(windows)]
fn apply_no_activate(w: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    if let Ok(h) = w.hwnd() {
        let hwnd = HWND(h.0);
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
            );
        }
    }
}

#[cfg(not(windows))]
fn apply_no_activate(_w: &WebviewWindow) {}

/// 启动听写引擎(只启动一次)。就绪 setup 与向导完成都经此入口。
fn start_engine(app: &tauri::AppHandle) {
    let started = app.state::<EngineStarted>();
    if started.0.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return; // 已启动过,忽略
    }
    let app_handle = app.clone();
    match Config::load_resolved() {
        Ok(cfg) => {
            let enabled = cfg.overlay.enabled;
            let observer = Arc::new(TauriObserver { app: app_handle, enabled });
            std::thread::spawn(move || {
                if let Err(e) = voice_input::engine::run_with(cfg, observer) {
                    eprintln!("引擎退出: {e}");
                }
            });
        }
        Err(e) => {
            eprintln!("加载配置失败: {e}");
            started.0.store(false, Ordering::SeqCst); // 回滚 once 守卫,避免损坏 config 时永久锁死
        }
    }
}

/// 向导「完成」:用向导填的 LLM 值更新(或创建)config.toml,然后当场启动引擎。
#[tauri::command]
fn finish_wizard(
    app: tauri::AppHandle,
    llm: voice_input::config::LlmConfig,
) -> Result<(), String> {
    // 读现有(无则默认),只覆盖 [llm],保留其它字段。
    let (mut cfg, path) = match Config::load_raw() {
        Ok((c, p)) => (c, p),
        Err(_) => {
            let dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .ok_or("无法确定程序目录")?;
            (Config::default(), dir.join("config.toml"))
        }
    };
    let mut llm = llm;
    if llm.api_key.trim().is_empty() {
        llm.enabled = false; // 没填 key 就不开 LLM,避免每次失败请求
    }
    cfg.llm = llm;
    cfg.save_to(&path).map_err(|e| format!("{e:#}"))?;
    start_engine(&app);
    // 向导完成 → 隐藏主窗口,转入托盘后台运行。
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        // 开机自启:Windows 走注册表 Run 键;MacosLauncher 参数仅 macOS 生效。
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .manage(ControlSlot::default())
        .manage(wizard::DownloadCancel::default())
        .manage(EngineStarted::default())
        .invoke_handler(tauri::generate_handler![
            cancel_recording,
            settings::get_config,
            settings::save_config,
            settings::test_llm,
            settings::restart_app,
            settings::open_config_dir,
            wizard::wizard_state,
            wizard::check_dependencies,
            wizard::open_external,
            wizard::download_model,
            wizard::cancel_download,
            wizard::import_model,
            finish_wizard
        ])
        .setup(|app| {
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings, &quit])?;
            let _tray = TrayIconBuilder::with_id("main")
                .tooltip("ByType")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "settings" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // 浮窗:加不抢焦点扩展样式
            if let Some(w) = app.get_webview_window("overlay") {
                apply_no_activate(&w);
            }

            // 就绪分流:就绪→启动引擎(主窗口保持隐藏到托盘);未就绪→显示主窗口跑首启向导。
            let handle = app.handle().clone();
            if wizard::wizard_state().ready {
                start_engine(&handle);
            } else if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running ByType");
}
