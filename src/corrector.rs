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

    /// 普通整理。禁用 / 文本过短 / 网络失败 / 空响应 → 返回原文(绝不丢输入)。
    pub fn correct(&self, raw: &str) -> String {
        self.process(raw, &self.cfg.effective_system_prompt())
    }

    /// 翻译(去语气词 + 译成英文)。同样在失败时回退原文。
    pub fn translate(&self, raw: &str) -> String {
        self.process(raw, &self.cfg.effective_translate_prompt())
    }

    /// 用给定系统提示词处理文本;任何不利情况都回退原文。
    fn process(&self, raw: &str, system_prompt: &str) -> String {
        let trimmed = raw.trim();
        if !self.cfg.enabled || trimmed.chars().count() < self.cfg.skip_if_shorter_than {
            return raw.to_string();
        }
        match self.try_process(trimmed, system_prompt) {
            Ok(t) if !t.trim().is_empty() => t,
            Ok(_) => raw.to_string(),
            Err(e) => {
                eprintln!("LLM 处理失败,回退原文: {e}");
                raw.to_string()
            }
        }
    }

    fn try_process(&self, raw: &str, system_prompt: &str) -> anyhow::Result<String> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let body = build_request_body(&self.cfg, system_prompt, raw);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.cfg.api_key)
            .json(&body)
            .send()?
            .error_for_status()?;
        let value: Value = resp.json()?;
        parse_response(&value)
            .ok_or_else(|| anyhow::anyhow!("响应缺少 choices[0].message.content"))
    }
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
        assert_eq!(corrector.correct("原始文本"), "原始文本");
    }

    #[test]
    fn too_short_returns_raw() {
        let mut c = cfg();
        c.enabled = true;
        c.skip_if_shorter_than = 10;
        let corrector = Corrector::new(c).unwrap();
        assert_eq!(corrector.correct("嗯"), "嗯");
    }
}
