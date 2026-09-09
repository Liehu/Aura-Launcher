//! Clarification Engine (P27-A06, spec §18): decide WHEN the agent must ask
//! the user a clarifying question instead of proceeding, and generate the
//! question. Deterministic policy over proposal confidence + intent
//! ambiguity — clarification is a UX safety valve, never an authority.

use crate::agent_contract::AgentIntent;
use crate::intent::Entities;

/// Policy thresholds (P27-A06 v1 defaults; configurable later via settings).
#[derive(Debug, Clone, Copy)]
pub struct ClarificationPolicy {
    /// Proposals below this confidence must ask the user.
    pub min_confidence: f32,
}

impl Default for ClarificationPolicy {
    fn default() -> Self {
        Self { min_confidence: 0.6 }
    }
}

/// The clarification the user sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clarification {
    pub question: String,
    pub reason: ClarificationReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClarificationReason {
    LowConfidence,
    AmbiguousTarget,
    EmptyInput,
}

/// Decide whether to proceed or ask. Returns Some(question) when the request
/// is ambiguous/low-confidence. Pure and deterministic.
pub fn needs_clarification(
    intent: AgentIntent,
    entities: &Entities,
    confidence: f32,
    policy: &ClarificationPolicy,
) -> Option<Clarification> {
    // empty input: nothing to act on
    let empty_input = match intent {
        AgentIntent::Search => entities.query.as_deref().unwrap_or("").trim().is_empty(),
        AgentIntent::Open | AgentIntent::Execute => {
            entities.target.as_deref().unwrap_or("").trim().is_empty()
        }
        AgentIntent::Workflow => {
            entities.workflow_id.as_deref().unwrap_or("").trim().is_empty()
        }
        AgentIntent::Explain | AgentIntent::Unknown => {
            entities.query.as_deref().unwrap_or("").trim().is_empty()
        }
    };
    if empty_input {
        return Some(Clarification {
            question: "你想做什么？请补充具体内容。".into(),
            reason: ClarificationReason::EmptyInput,
        });
    }
    // ambiguous verb: "run" without a concrete runnable target name
    if intent == AgentIntent::Execute
        && entities
            .target
            .as_deref()
            .is_some_and(|t| t.len() < 3)
    {
        return Some(Clarification {
            question: format!("`{}` 不是一个明确的可执行目标——你想运行什么？", entities.target.as_deref().unwrap_or("")),
            reason: ClarificationReason::AmbiguousTarget,
        });
    }
    if confidence < policy.min_confidence {
        return Some(Clarification {
            question: "我不太确定你的意图——可以再说得具体一点吗？".into(),
            reason: ClarificationReason::LowConfidence,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_asks() {
        let pol = ClarificationPolicy::default();
        let q = needs_clarification(AgentIntent::Search, &Entities::default(), 0.9, &pol);
        assert!(q.is_some());
        assert_eq!(q.unwrap().reason, ClarificationReason::EmptyInput);
    }

    #[test]
    fn ambiguous_execute_target_asks() {
        let pol = ClarificationPolicy::default();
        let e = Entities {
            target: Some("x".into()),
            ..Default::default()
        };
        let q = needs_clarification(AgentIntent::Execute, &e, 0.9, &pol);
        assert_eq!(q.unwrap().reason, ClarificationReason::AmbiguousTarget);
    }

    #[test]
    fn low_confidence_asks_high_confidence_proceeds() {
        let pol = ClarificationPolicy::default();
        let e = Entities {
            query: Some("quarterly report".into()),
            ..Default::default()
        };
        assert!(needs_clarification(AgentIntent::Search, &e, 0.3, &pol).is_some());
        assert!(needs_clarification(AgentIntent::Search, &e, 0.9, &pol).is_none());
    }

    #[test]
    fn deterministic() {
        let pol = ClarificationPolicy::default();
        let e = Entities {
            query: Some("report".into()),
            ..Default::default()
        };
        let first = needs_clarification(AgentIntent::Search, &e, 0.5, &pol);
        for _ in 0..100 {
            assert_eq!(needs_clarification(AgentIntent::Search, &e, 0.5, &pol), first);
        }
    }
}
