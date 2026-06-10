// release 构建用 windows 子系统,避免弹出控制台黑窗(关掉黑窗会杀进程)。
// debug(tauri dev)保留控制台,便于看引擎日志。请勿删除。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    bytype_lib::run();
}
