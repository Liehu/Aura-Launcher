//! Risk Classifier (P27-B06, spec `P2.7 开发设计规范` §19): deterministic
//! classification of plan steps into RiskLevel L0-L4. Classification is an
//! AI-side HINT that can only make approval MORE likely — the actual gate
//! stays ActionResolver/Policy.

use crate::agent_contract::{PlanStep, RiskLevel};

/// Classify one plan step by its logical action_ref (§19 table).
pub fn classify(step: &PlanStep) -> RiskLevel {
    let r = &step.action_ref;
    if r.starts_with("explain:") || r.starts_with("search:") {
        return RiskLevel::L0Informational;
    }
    if r.starts_with("command:file:") || r.starts_with("command:app:") {
        // opening/reading is read-only-ish; copy is reversible
        if r.contains("copy") {
            return RiskLevel::L2Reversible;
        }
        return RiskLevel::L1ReadOnly;
    }
    if r.starts_with("clipboard:") {
        return RiskLevel::L2Reversible;
    }
    if r.starts_with("workflow:") {
        return RiskLevel::L2Reversible;
    }
    if r.starts_with("shell:") || r.starts_with("power:") {
        return RiskLevel::L3Destructive;
    }
    if r.starts_with("privileged:") {
        return RiskLevel::L4Sensitive;
    }
    // unknown vocabulary = assume the worst (fail-closed)
    RiskLevel::L4Sensitive
}

/// Convenience: does this step require approval per its classification?
pub fn requires_approval(step: &PlanStep) -> bool {
    classify(step).requires_approval()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(action_ref: &str) -> PlanStep {
        PlanStep {
            step_id: "s".into(),
            action_ref: action_ref.into(),
            input: json!({}),
            rationale: None,
            requires_approval: false,
        }
    }

    /// §19 table: L0/L1 auto, L3/L4 forced approval.
    #[test]
    fn classification_matches_default_policy() {
        assert_eq!(classify(&step("search:reports")), RiskLevel::L0Informational);
        assert!(!requires_approval(&step("search:reports")));

        assert_eq!(classify(&step("command:app:chrome")), RiskLevel::L1ReadOnly);
        assert!(!requires_approval(&step("command:app:chrome")));

        assert_eq!(classify(&step("clipboard:copy")), RiskLevel::L2Reversible);

        assert_eq!(classify(&step("shell:run_script")), RiskLevel::L3Destructive);
        assert!(requires_approval(&step("shell:run_script")));

        assert_eq!(classify(&step("privileged:admin_task")), RiskLevel::L4Sensitive);
        assert!(requires_approval(&step("privileged:admin_task")));
    }

    /// Unknown vocabulary fails closed to L4 (the most approval-demanding).
    #[test]
    fn unknown_refs_fail_closed() {
        assert_eq!(classify(&step("mystery:voodoo")), RiskLevel::L4Sensitive);
        assert!(requires_approval(&step("mystery:voodoo")));
    }
}
