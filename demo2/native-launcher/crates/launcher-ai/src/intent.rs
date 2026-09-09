//! Intent/Entity model (P27-A03, spec `P2.7 开发设计规范` §9): deterministic
//! natural-language parsing into an AgentIntent + typed entities. Rule-based
//! v0.1 — NO LLM, NO network, NO I/O; an LLM may later REPLACE the parser
//! behind the same output contract (structured output validated identically).
//!
//! Intent/entities are proposal data: they never carry execution authority
//! (the AgentProposal contract in agent_contract.rs stays the gate).

use serde::{Deserialize, Serialize};

/// Typed entities extracted from natural language (§9 entity model).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entities {
    /// Free-text search subject ("find quarterly report" → the subject).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Named target of open/run/execute verbs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Workflow id for workflow requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRequest {
    pub intent: crate::agent_contract::AgentIntent,
    pub entities: Entities,
}

/// Deterministic rule-based parse. Verb-led patterns take priority; anything
/// unmatched falls back to a Search intent with the whole input as query —
/// natural language can never be misread as a privileged operation here
/// because the result is always a PROPOSAL input.
pub fn parse_intent(input: &str) -> ParsedRequest {
    let trimmed = input.trim();
    let lower = trimmed.to_lowercase();
    let mut entities = Entities::default();

    let (intent, rest) = if let Some(rest) = lower.strip_prefix("find ") {
        (crate::agent_contract::AgentIntent::Search, rest)
    } else if let Some(rest) = lower.strip_prefix("search ") {
        (crate::agent_contract::AgentIntent::Search, rest)
    } else if let Some(rest) = lower.strip_prefix("open ") {
        (crate::agent_contract::AgentIntent::Open, rest)
    } else if let Some(rest) = lower.strip_prefix("run ") {
        (crate::agent_contract::AgentIntent::Execute, rest)
    } else if let Some(rest) = lower.strip_prefix("workflow ") {
        (crate::agent_contract::AgentIntent::Workflow, rest)
    } else {
        (
            crate::agent_contract::AgentIntent::Search,
            lower.as_str(),
        )
    };

    match intent {
        crate::agent_contract::AgentIntent::Search => {
            entities.query = Some(rest.trim().to_string());
        }
        crate::agent_contract::AgentIntent::Open => {
            entities.target = Some(rest.trim().to_string());
        }
        crate::agent_contract::AgentIntent::Execute => {
            entities.target = Some(rest.trim().to_string());
        }
        crate::agent_contract::AgentIntent::Workflow => {
            entities.workflow_id = Some(rest.trim().to_string());
        }
        _ => {}
    }
    ParsedRequest { intent, entities }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_contract::AgentIntent;

    #[test]
    fn verb_patterns_route_intents() {
        assert_eq!(parse_intent("find quarterly report").intent, AgentIntent::Search);
        assert_eq!(
            parse_intent("find quarterly report").entities.query,
            Some("quarterly report".into())
        );
        assert_eq!(parse_intent("open chrome").intent, AgentIntent::Open);
        assert_eq!(parse_intent("open chrome").entities.target, Some("chrome".into()));
        assert_eq!(parse_intent("run backup").intent, AgentIntent::Execute);
        assert_eq!(
            parse_intent("workflow nightly-backup").entities.workflow_id,
            Some("nightly-backup".into())
        );
    }

    /// Unmatched input falls back to Search with the full text (safe default).
    #[test]
    fn fallback_is_search() {
        let p = parse_intent("quarterly report 2026");
        assert_eq!(p.intent, AgentIntent::Search);
        assert_eq!(p.entities.query, Some("quarterly report 2026".into()));
    }

    /// Determinism + case-insensitivity.
    #[test]
    fn deterministic_and_case_insensitive() {
        let a = parse_intent("Open CHROME");
        let b = parse_intent("open chrome");
        assert_eq!(a, b);
        for _ in 0..100 {
            assert_eq!(parse_intent("Open CHROME"), a);
        }
    }

    /// Empty input = Search with empty query (never panics).
    #[test]
    fn empty_input_safe() {
        let p = parse_intent("");
        assert_eq!(p.intent, AgentIntent::Search);
        assert_eq!(p.entities.query, Some("".into()));
    }
}
