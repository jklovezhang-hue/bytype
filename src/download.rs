//! 带进度回调的文件下载(reqwest blocking streaming)。
//! 供首启向导下载模型用;下载逻辑放核心、进度与取消由调用方注入(与 GUI 解耦)。

use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result};

/// 下载 `url` 到 `dest`,边下边回调 `on_progress(received, total)`(total=0 表示未知)。
/// 每当 `should_cancel()` 返回 true 立即中止并返回错误(调用方负责删残文件)。
pub fn download_file(
    url: &str,
    dest: &Path,
    mut on_progress: impl FnMut(u64, u64),
    should_cancel: impl Fn() -> bool,
) -> Result<()> {
    // 大文件:不设总超时(只在建连阶段用默认行为)。
    let client = reqwest::blocking::Client::builder().build()?;
    let mut resp = client
        .get(url)
        .send()
        .with_context(|| format!("请求失败: {url}"))?
        .error_for_status()
        .with_context(|| format!("下载响应错误: {url}"))?;
    let total = resp.content_length().unwrap_or(0);

    let mut file =
        std::fs::File::create(dest).with_context(|| format!("创建文件失败: {}", dest.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    loop {
        if should_cancel() {
            anyhow::bail!("已取消");
        }
        let n = resp.read(&mut buf).context("读取下载流失败")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).context("写入文件失败")?;
        received += n as u64;
        on_progress(received, total);
    }
    file.flush().ok();
    Ok(())
}
