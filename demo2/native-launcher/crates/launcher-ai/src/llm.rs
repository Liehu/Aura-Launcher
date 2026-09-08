//! LLM provider abstraction (review 55 §5/§37/§20): sync by design —
//! "async is an implementation detail; not blocking the UI is an
//! architecture requirement". Callers run planners on worker threads.
//!
//! `LlmResponse` carries TEXT only. There is deliberately no way to
//! express Effects/ResolvedActions at this layer (§37): the type system
//! blocks authority leakage before any parsing happens.

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LlmError {
    #[error("llm provider unavailable: {0}")]
    Unavailable(String),
    #[error("llm request timed out")]
    Timeout,
    #[error("llm returned invalid output")]
    InvalidOutput(String),
}

/// A text-in/text-out LLM backend. Implementations: deterministic mock
/// (tests/CI) and OpenAI-compatible chat (local Ollama / remote endpoints).
pub trait LlmProvider: Send + Sync {
    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse, LlmError>;
    /// Human-readable backend identity for planner diagnostics only.
    fn name(&self) -> &str;
}

/// Deterministic mock (review 55 §42): scripted per-intent responses so CI
/// never touches an external API. Returns the response whose `when`
/// substring appears in the user prompt, else the first entry.
pub struct MockLlmProvider {
    name: String,
    scripted: Vec<(String, String)>,
}

impl MockLlmProvider {
    pub fn new(scripted: Vec<( impl Into<String>, impl Into<String> )>) -> Self {
        Self {
            name: "mock".into(),
            scripted: scripted
                .into_iter()
                .map(|(w, r)| (w.into(), r.into()))
                .collect(),
        }
    }
}

impl LlmProvider for MockLlmProvider {
    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse, LlmError> {
        for (when, response) in &self.scripted {
            if request.user_prompt.contains(when.as_str()) {
                return Ok(LlmResponse { text: response.clone() });
            }
        }
        match self.scripted.first() {
            Some((_, r)) => Ok(LlmResponse { text: r.clone() }),
            None => Err(LlmError::Unavailable("mock has no scripted response".into())),
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
}

/// OpenAI-compatible chat-completions provider (Ollama, vLLM, remote
/// endpoints). The API key is supplied programmatically (credential-store
/// sourced) and is NEVER part of config.toml (review 55 §38).
pub struct OpenAiCompatibleProvider {
    /// e.g. `http://127.0.0.1:11434/v1` (Ollama) — HTTPS required for
    /// non-loopback endpoints.
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub timeout: std::time::Duration,
}

impl LlmProvider for OpenAiCompatibleProvider {
    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse, LlmError> {
        // loopback http allowed; everything else must be https (same policy
        // family as the MCP transport URL boundary)
        let host = self
            .base_url
            .split_once("://")
            .map(|(_, rest)| rest.split('/').next().unwrap_or_default())
            .unwrap_or_default();
        let insecure = self.base_url.starts_with("http://")
            && !(host.starts_with("127.0.0.1") || host.starts_with("localhost"));
        if insecure {
            return Err(LlmError::Unavailable(
                "non-loopback plaintext http llm endpoints are rejected".into(),
            ));
        }
        let agent = ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .redirects(0)
            .build();
        let mut req = agent
            .post(&format!("{}/chat/completions", self.base_url.trim_end_matches('/')))
            .set("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": request.system_prompt},
                {"role": "user", "content": request.user_prompt}
            ],
            "stream": false
        });
        let response = req
            .send_string(&body.to_string())
            .map_err(|e| match e {
                ureq::Error::Status(code, _) => LlmError::Unavailable(format!("http {code}")),
                ureq::Error::Transport(t) => {
                    if t.to_string().contains("timed out") {
                        LlmError::Timeout
                    } else {
                        LlmError::Unavailable(t.to_string())
                    }
                }
            })?;
        let v: serde_json::Value = serde_json::from_reader(response.into_reader())
            .map_err(|e| LlmError::InvalidOutput(e.to_string()))?;
        let text = v["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| LlmError::InvalidOutput("missing choices[0].message.content".into()))?
            .to_string();
        Ok(LlmResponse { text })
    }

    fn name(&self) -> &str {
        &self.model
    }
}
