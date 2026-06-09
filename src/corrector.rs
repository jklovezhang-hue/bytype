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

    fn try_chat(&self, system_prompt: &str, user_text: &str) -> anyhow::Result<String> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let body = build_request_body(&self.cfg, system_prompt, user_text);
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
}
