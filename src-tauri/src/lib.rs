mod settings;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(ControlSlot::default())
        .invoke_handler(tauri::generate_handler![
            cancel_recording,
            settings::get_config,
            settings::save_config,
            settings::test_llm,
            settings::restart_app,
            settings::open_config_dir
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

            // 读配置并在后台线程跑引擎(把状态回调到浮窗)
            let app_handle = app.handle().clone();
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
                Err(e) => eprintln!("加载配置失败: {e}"),
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
