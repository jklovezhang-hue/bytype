//! LLM 纠错。调用 OpenAI 兼容的 /chat/completions;失败/超时/禁用/过短均回退原文。

use std::time::Duration;

use serde_json::{json, Value};

use crate::config::LlmConfig;

pub struct Corrector {
    client: reqwest::blocking::Client,
    cfg: LlmConfig,
}

impl Corrector {
    pub fn new(cfg: LlmConfig) -> anyhow::Result<Corrector> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs.max(1)))
            .build()?;
        Ok(Corrector { client, cfg })
    }

    /// 普通整理。`style` 为可选的应用风格指令。失败回退原文。
    pub fn correct(&self, raw: &str, style: Option<&str>) -> String {
        let sys = compose_system_prompt(
            &self.cfg.effective_system_prompt(),
            self.cfg.vocabulary_line().as_deref(),
            style,
        );
        self.process(raw, &sys)
    }

    /// 翻译成英文。`style` 为可选的应用风格指令。失败回退原文。
    pub fn translate(&self, raw: &str, style: Option<&str>) -> String {
        let sys = compose_system_prompt(
            &self.cfg.effective_translate_prompt(),
            self.cfg.vocabulary_line().as_deref(),
            style,
        );
        self.process(raw, &sys)
    }

    /// 命令模式:把 `instruction` 应用到 `selected`。失败回退原选中文本。
    pub fn command(&self, instruction: &str, selected: &str) -> String {
        if !self.cfg.enabled {
            return selected.to_string();
        }
        let sys = compose_system_prompt(
            &self.cfg.effective_command_prompt(),
            self.cfg.vocabulary_line().as_deref(),
            None,
        );
        let user = format!("指令:{}\n\n文本:\n{}", instruction.trim(), selected);
        match self.try_chat(&sys, &user) {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) => selected.to_string(),
            Err(e) => {
                eprintln!("LLM 命令失败,保留原文: {e}");
                selected.to_string()
            }
        }
    }

    /// 会议转写逐段清理:用「clean」预设(忠实清理,不改写)+ 词库。失败/禁用回退原文。
    pub fn clean_line(&self, text: &str) -> String {
        let sys = compose_system_prompt(
            &crate::config::preset_prompt("clean"),
            self.cfg.vocabulary_line().as_deref(),
            None,
        );
        self.process(text, &sys)
    }

    /// 会议整段清理:与 `clean_line` 同(clean 预设 + 词库),但**清理后为空不回退原文**
    /// —— 让纯语气词段(只含「嗯/yeah」之类)清成空串,由调用方丢弃该段。
    /// 关闭 LLM 时返回原文;**API 调用失败时也返回原文**(只在确实清成空时才返回空)。
    pub fn clean_for_meeting(&self, text: &str) -> String {
        if !self.cfg.enabled {
            return text.to_string();
        }
        let sys = compose_system_prompt(
            &crate::config::preset_prompt("clean"),
            self.cfg.vocabulary_line().as_deref(),
            None,
        );
        match self.try_chat(&sys, text.trim()) {
            Ok(t) => t.trim().to_string(), // 可能为空 → 调用方丢弃该段
            Err(e) => {
                eprintln!("会议转写清理失败,保留原文: {e}");
                text.to_string()
            }
        }
    }

    /// 用给定系统提示词处理文本(用户消息即文本本身);失败回退原文。
    fn process(&self, raw: &str, system_prompt: &str) -> String {
        let trimmed = raw.trim();
        if !self.cfg.enabled || trimmed.chars().count() < self.cfg.skip_if_shorter_than {
            return raw.to_string();
        }
        match self.try_chat(system_prompt, trimmed) {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) => raw.to_string(),
            Err(e) => {
                eprintln!("LLM 处理失败,回退原文: {e}");
                raw.to_string()
            }
        }
    }

    /// 发一次 chat/completions;对**瞬时**错误(连接/超时/发送失败、5xx 服务端错误)做指数退避重试。
    /// 会议转写会在后台连发多段清理请求,relay 偶发丢连接("error sending request")会导致该段
    /// 回退原文(错字/语气词残留),重试可显著降低这种漏清理。4xx 与响应解析错误不重试。
    fn try_chat(&self, system_prompt: &str, user_text: &str) -> anyhow::Result<String> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let body = build_request_body(&self.cfg, system_prompt, user_text);
        let mut attempt = 0u32;
        loop {
            let resp = match self
                .client
                .post(&url)
                .bearer_auth(&self.cfg.api_key)
                .json(&body)
                .send()
            {
                Ok(r) => r,
                Err(e) => {
                    if (e.is_connect() || e.is_timeout() || e.is_request()) && attempt < 2 {
                        std::thread::sleep(Duration::from_millis(300 * (1 << attempt)));
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            };
            let resp = match resp.error_for_status() {
                Ok(r) => r,
                Err(e) => {
                    // 5xx 服务端错误可重试;4xx(鉴权/参数)直接失败。
                    if e.status().map(|s| s.is_server_error()).unwrap_or(false) && attempt < 2 {
                        std::thread::sleep(Duration::from_millis(300 * (1 << attempt)));
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            };
            let value: Value = resp.json()?;
            return parse_response(&value)
                .ok_or_else(|| anyhow::anyhow!("响应缺少 choices[0].message.content"));
        }
    }
}

/// 连通性测试:用给定 [llm] 配置发一条固定请求,返回(耗时 ms, 回复文本)。
/// 供设置界面"测试连接"按钮用:**不受** `enabled` 与 `skip_if_shorter_than` 影响,
/// temperature 固定 0;失败时原样返回错误(由调用方展示)。
pub fn test_connection(cfg: &LlmConfig) -> anyhow::Result<(u64, String)> {
    if cfg.base_url.trim().is_empty() {
        anyhow::bail!("接口地址不能为空");
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cfg.timeout_secs.max(1)))
        .build()?;
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = json!({
        "model": cfg.model,
        "temperature": 0.0,
        "messages": [
            { "role": "system", "content": "你是连接测试助手,请只回复:你好,ByType!" },
            { "role": "user", "content": "ping" },
        ],
    });
    let start = std::time::Instant::now();
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()?
        .error_for_status()?;
    let value: Value = resp.json()?;
    let reply = parse_response(&value)
        .ok_or_else(|| anyhow::anyhow!("响应缺少 choices[0].message.content"))?;
    Ok((start.elapsed().as_millis() as u64, reply))
}

/// 生成会议纪要:用 [llm] 配置 + 给定纪要提示词,把整段转写作为用户消息发给 LLM。
/// 超时取 max(120, timeout_secs)。失败返回 Err。不受 enabled/skip 影响。
pub fn generate_minutes(cfg: &LlmConfig, prompt: &str, content: &str) -> anyhow::Result<String> {
    if cfg.base_url.trim().is_empty() {
        anyhow::bail!("未配置 LLM 接口地址");
    }
    let secs = cfg.timeout_secs.max(120);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(secs))
        .build()?;
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = build_request_body(cfg, prompt, content);
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .send()?
        .error_for_status()?;
    let value: Value = resp.json()?;
    parse_response(&value).ok_or_else(|| anyhow::anyhow!("响应缺少 choices[0].message.content"))
}

/// 把词库行、应用风格依次拼到基础系统提示词后面(空项跳过)。
pub fn compose_system_prompt(
    base: &str,
    vocabulary_line: Option<&str>,
    style: Option<&str>,
) -> String {
    let mut s = base.to_string();
    for extra in [vocabulary_line, style].into_iter().flatten() {
        if !extra.trim().is_empty() {
            s.push_str("\n\n");
            s.push_str(extra.trim());
        }
    }
    s
}

/// 构造 chat/completions 请求体。
pub fn build_request_body(cfg: &LlmConfig, system_prompt: &str, raw: &str) -> Value {
    json!({
        "model": cfg.model,
        "temperature": cfg.temperature,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": raw },
        ],
    })
}

/// 从 chat/completions 响应里取 choices[0].message.content。
pub fn parse_response(value: &Value) -> Option<String> {
    value
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;

    fn cfg() -> LlmConfig {
        LlmConfig {
            model: "deepseek-v4-flash".into(),
            temperature: 0.2,
            system_prompt: "SP".into(),
            ..Default::default()
        }
    }

    #[test]
    fn request_body_has_model_and_two_messages() {
        let body = build_request_body(&cfg(), "SYS", "你好");
        assert_eq!(body["model"], "deepseek-v4-flash");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "SYS");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "你好");
    }

    #[test]
    fn parse_response_extracts_content() {
        let v: Value = serde_json::from_str(
            r#"{"choices":[{"message":{"role":"assistant","content":"整理后的文本"}}]}"#,
        )
        .unwrap();
        assert_eq!(parse_response(&v).as_deref(), Some("整理后的文本"));
    }

    #[test]
    fn parse_response_missing_content_is_none() {
        let v: Value = serde_json::from_str(r#"{"choices":[]}"#).unwrap();
        assert_eq!(parse_response(&v), None);
    }

    #[test]
    fn disabled_returns_raw() {
        let mut c = cfg();
        c.enabled = false;
        let corrector = Corrector::new(c).unwrap();
        assert_eq!(corrector.correct("原始文本", None), "原始文本");
    }

    #[test]
    fn too_short_returns_raw() {
        let mut c = cfg();
        c.enabled = true;
        c.skip_if_shorter_than = 10;
        let corrector = Corrector::new(c).unwrap();
        assert_eq!(corrector.correct("嗯", None), "嗯");
    }

    #[test]
    fn compose_appends_vocab_and_style() {
        let s = compose_system_prompt("BASE", Some("VOCAB"), Some("STYLE"));
        assert!(s.starts_with("BASE"));
        assert!(s.contains("VOCAB"));
        assert!(s.contains("STYLE"));
    }

    #[test]
    fn compose_skips_empty() {
        assert_eq!(compose_system_prompt("BASE", None, Some("  ")), "BASE");
    }

    #[test]
    fn command_disabled_returns_selected() {
        let mut c = cfg();
        c.enabled = false;
        let corrector = Corrector::new(c).unwrap();
        assert_eq!(corrector.command("改短", "一段很长的文本"), "一段很长的文本");
    }

    #[test]
    fn clean_line_disabled_returns_raw() {
        let mut c = cfg();
        c.enabled = false;
        let corrector = Corrector::new(c).unwrap();
        assert_eq!(corrector.clean_line("嗯那个文本啊"), "嗯那个文本啊");
    }
}
