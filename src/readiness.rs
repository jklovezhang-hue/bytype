//! 就绪检测:语音识别模型文件是否齐全。

use std::path::Path;

/// 模型是否齐全:`model_dir` 下 `model.onnx` 与 `tokens.txt` 都存在且非空。
pub fn model_present(model_dir: &Path) -> bool {
    let ok = |name: &str| {
        std::fs::metadata(model_dir.join(name))
            .map(|m| m.is_file() && m.len() > 0)
            .unwrap_or(false)
    };
    ok("model.onnx") && ok("tokens.txt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_only_when_both_files_nonempty() {
        let dir = std::env::temp_dir().join(format!("bytype-g5-readiness-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 都没有 → false
        assert!(!model_present(&dir));
        // 只有 model.onnx → false
        std::fs::write(dir.join("model.onnx"), b"x").unwrap();
        assert!(!model_present(&dir));
        // 两个都在且非空 → true
        std::fs::write(dir.join("tokens.txt"), b"y").unwrap();
        assert!(model_present(&dir));
        // tokens.txt 为空 → false
        std::fs::write(dir.join("tokens.txt"), b"").unwrap();
        assert!(!model_present(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }
}
