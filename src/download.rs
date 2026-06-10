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
    // timeout:总请求超时上限,防网络挂起时 read() 永久阻塞(取消标志靠循环轮询)。
    // reqwest 0.12 blocking::ClientBuilder 无 read_timeout,改用 timeout(1800s 够下完 228MB)。
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1800))
        .build()?;
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
    file.flush().context("刷新文件失败")?;
    Ok(())
}
