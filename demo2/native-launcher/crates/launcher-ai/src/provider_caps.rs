//! Provider capabilities + health check (P27-A01, spec `P2.7 开发设计规范`
//! §28-§29): model capability declaration and deterministic health probing.
//! Host routing uses these to pick a provider BEFORE spending a call —
//! capability data is advisory, never authority.

use crate::llm::{LlmError, LlmProvider, LlmRequest};

use serde::{Deserialize, Serialize};

/// §29 Model Capability: what a provider/backend can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    StructuredJson,
    Streaming,
    LongContext,
    LocalInference,
}

/// A provider's declared capability set + identity, reported by the host
/// (not by the model itself — self-reported data is untrusted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProfile {
    pub name: String,
    pub capabilities: Vec<ModelCapability>,
}

impl ProviderProfile {
    pub fn has(&self, cap: ModelCapability) -> bool {
        self.capabilities.contains(&cap)
    }
}

/// A01 health probe: verify a provider can answer a trivial request within
/// tolerance. Returns Ok(()) or the error. Never retried here (the caller
/// owns retry policy).
pub fn health_check(
    provider: &dyn LlmProvider,
    probe: &LlmRequest,
) -> Result<(), LlmError> {
    provider.generate(probe).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockLlmProvider;

    /// A01: mock provider passes health check.
    #[test]
    fn mock_provider_health_ok() {
        let provider = MockLlmProvider::new(vec![("ping", "pong")]);
        let probe = LlmRequest {
            system_prompt: String::new(),
            user_prompt: "ping".into(),
        };
        assert!(health_check(&provider, &probe).is_ok());
    }

    /// Capability matching: routing picks a provider with the required cap.
    #[test]
    fn capability_matching() {
        let p = ProviderProfile {
            name: "local-mock".into(),
            capabilities: vec![ModelCapability::StructuredJson, ModelCapability::LocalInference],
        };
        assert!(p.has(ModelCapability::StructuredJson));
        assert!(!p.has(ModelCapability::Streaming));
        assert!(p.has(ModelCapability::LocalInference));
    }
}
