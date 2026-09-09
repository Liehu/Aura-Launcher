//! Plan Schema (P27-B02, spec `P2.7 开发设计规范` §8/§39-B02): a versioned,
//! host-owned plan document that survives the LLM turn — the currency for
//! plan editing (D04), persistence and the validator (B03). The LLM never
//! produces this type directly; it is derived from a validated
//! [`AgentProposal`] (A05 is the only untrusted-JSON entry).

use serde::{Deserialize, Serialize};

use crate::agent_contract::{AgentProposal, PlanStep};

/// Schema revision. Unknown revisions fail closed on load (same policy
/// family as MCP wire profiles / install plans).
pub const PLAN_SCHEMA_VERSION: u32 = 1;

/// A host-side plan document: a validated proposal's plan, plus the
/// provenance needed for editing and audit. DATA, never authorization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanDocument {
    /// `PLAN_SCHEMA_VERSION` at creation; load() rejects anything else.
    pub schema_version: u32,
    pub proposal_id: String,
    pub session_id: String,
    pub user_goal: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanDocumentError {
    /// Version is not the current schema revision (fail closed).
    UnknownVersion { got: u32, expected: u32 },
    /// Structural validation failed (delegates to the contract rules).
    Invalid(String),
}

impl std::fmt::Display for PlanDocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVersion { got, expected } => {
                write!(f, "unknown plan schema version {got} (expected {expected})")
            }
            Self::Invalid(e) => write!(f, "invalid plan document: {e}"),
        }
    }
}

impl PlanDocument {
    /// Derive a document from an already-validated proposal. Cheap and
    /// infallible: the contract validator ran at proposal construction.
    pub fn from_proposal(p: &AgentProposal) -> Self {
        Self {
            schema_version: PLAN_SCHEMA_VERSION,
            proposal_id: p.proposal_id.clone(),
            session_id: p.session_id.clone(),
            user_goal: p.user_goal.clone(),
            steps: p.plan.clone(),
        }
    }

    /// Structural validation: same fail-closed rules as the proposal
    /// contract (non-empty plan, unique bounded step ids, non-empty refs),
    /// applied to the (possibly hand-edited) document.
    pub fn validate(&self) -> Result<(), PlanDocumentError> {
        if self.schema_version != PLAN_SCHEMA_VERSION {
            return Err(PlanDocumentError::UnknownVersion {
                got: self.schema_version,
                expected: PLAN_SCHEMA_VERSION,
            });
        }
        if self.proposal_id.is_empty() || self.proposal_id.len() > 128 {
            return Err(PlanDocumentError::Invalid("invalid proposal_id".into()));
        }
        if self.session_id.is_empty() || self.session_id.len() > 128 {
            return Err(PlanDocumentError::Invalid("invalid session_id".into()));
        }
        if self.user_goal.trim().is_empty() || self.user_goal.len() > 4096 {
            return Err(PlanDocumentError::Invalid("invalid user_goal".into()));
        }
        if self.steps.is_empty() {
            return Err(PlanDocumentError::Invalid("plan must have at least one step".into()));
        }
        if self.steps.len() > 64 {
            return Err(PlanDocumentError::Invalid("plan exceeds 64 steps".into()));
        }
        let mut seen = std::collections::HashSet::new();
        for s in &self.steps {
            if s.step_id.is_empty() || s.step_id.len() > 128 {
                return Err(PlanDocumentError::Invalid(format!(
                    "invalid step_id: {}",
                    s.step_id
                )));
            }
            if !seen.insert(s.step_id.clone()) {
                return Err(PlanDocumentError::Invalid(format!(
                    "duplicate step_id: {}",
                    s.step_id
                )));
            }
            if s.action_ref.trim().is_empty() || s.action_ref.len() > 256 {
                return Err(PlanDocumentError::Invalid(format!(
                    "invalid action_ref: {}",
                    s.action_ref
                )));
            }
        }
        Ok(())
    }

    /// Parse from host-side storage (D04 edited plans, session resume).
    /// Unknown schema versions fail closed.
    pub fn from_json(raw: &str) -> Result<Self, PlanDocumentError> {
        let doc: Self = serde_json::from_str(raw)
            .map_err(|e| PlanDocumentError::Invalid(e.to_string()))?;
        doc.validate()?;
        Ok(doc)
    }

    pub fn to_json(&self) -> Result<String, PlanDocumentError> {
        self.validate()?;
        serde_json::to_string(self)
            .map_err(|e| PlanDocumentError::Invalid(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> PlanDocument {
        PlanDocument {
            schema_version: PLAN_SCHEMA_VERSION,
            proposal_id: "p1".into(),
            session_id: "s1".into(),
            user_goal: "goal".into(),
            steps: vec![PlanStep {
                step_id: "s1".into(),
                action_ref: "p|c|a".into(),
                input: json!({}),
                rationale: None,
                requires_approval: false,
            }],
        }
    }

    /// B02: versioned round-trip through host-side JSON.
    #[test]
    fn json_roundtrip() {
        let d = doc();
        let json = d.to_json().unwrap();
        let back = PlanDocument::from_json(&json).unwrap();
        assert_eq!(back, d);
    }

    /// B02: unknown schema versions fail closed on load.
    #[test]
    fn unknown_version_fails_closed() {
        let raw = serde_json::json!({
            "schema_version": 99,
            "proposal_id": "p1", "session_id": "s1", "user_goal": "g",
            "steps": []
        })
        .to_string();
        assert!(matches!(
            PlanDocument::from_json(&raw),
            Err(PlanDocumentError::UnknownVersion { .. })
        ));
    }

    /// B02: structural rules hold on hand-edited documents (duplicate ids,
    /// empty plan).
    #[test]
    fn edited_document_validated() {
        let mut d = doc();
        d.steps.push(PlanStep {
            step_id: "s1".into(),
            action_ref: "p|c|b".into(),
            input: json!({}),
            rationale: None,
            requires_approval: false,
        });
        assert!(matches!(d.validate(), Err(PlanDocumentError::Invalid(_))));
        d.steps.clear();
        assert!(d.validate().is_err());
    }
}
