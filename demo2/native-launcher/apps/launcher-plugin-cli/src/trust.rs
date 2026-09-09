//! Trust Model + lifecycle management (P2.8-B/C, spec
//! `P2.8 — Ecosystem & Distribution 1.0 技术设计规范.md` §8/§9/§35).
//!
//! §8 fail-closed: an unsigned sideloaded package is NEVER auto-trusted —
//! the best it can get is "requires explicit user approval". Signature
//! verification itself is the GA-6 external-cert interface: this model
//! consumes a SignatureStatus, it does not verify certificates.

use crate::foundation::{transition_allowed, PluginLifecycle};

/// §9 signature status (the GA-6 interface: a real certificate check lands
/// behind this enum when a signing certificate exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    Signed,
    ChecksumVerified,
    Unsigned,
}

/// §12/§41 where the package came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSource {
    OfficialRepository,
    SideLoad,
}

/// §8 trust decision — DATA for the approval flow, never an authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustDecision {
    Trusted,
    RequiresExplicitApproval,
    Untrusted,
}

/// Deterministic, fail-closed trust evaluation:
/// - Signed packages from the official repository → Trusted;
/// - Signed sideloads / checksum-verified official packages → explicit
///   approval required (usable, but the user must see what they approved);
/// - unsigned sideloads → Untrusted (install refused by default policy).
pub fn evaluate_trust(signature: SignatureStatus, source: PackageSource) -> TrustDecision {
    match (signature, source) {
        (SignatureStatus::Signed, PackageSource::OfficialRepository) => TrustDecision::Trusted,
        (SignatureStatus::Signed, PackageSource::SideLoad)
        | (SignatureStatus::ChecksumVerified, _) => TrustDecision::RequiresExplicitApproval,
        (SignatureStatus::Unsigned, _) => TrustDecision::Untrusted,
    }
}

/// C/§35 lifecycle manager: gates every lifecycle change through the
/// transition whitelist — a Broken plugin can never silently come back to
/// Enabled, and Uninstalled plugins cannot skip installation.
#[derive(Debug, Default)]
pub struct LifecycleManager {
    states: std::collections::HashMap<String, PluginLifecycle>,
}

impl LifecycleManager {
    pub fn state(&self, id: &str) -> PluginLifecycle {
        self.states
            .get(id)
            .copied()
            .unwrap_or(PluginLifecycle::Uninstalled)
    }

    /// Apply a transition if legal; returns the new state.
    pub fn apply(
        &mut self,
        id: &str,
        requested: PluginLifecycle,
    ) -> Result<PluginLifecycle, String> {
        let current = self.state(id);
        if !transition_allowed(current, requested) {
            return Err(format!(
                "illegal lifecycle transition for `{id}`: {} -> {}",
                current.as_str(),
                requested.as_str()
            ));
        }
        self.states.insert(id.to_string(), requested);
        Ok(requested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §8 fail-closed matrix: only Signed+Official is auto-trusted.
    #[test]
    fn trust_matrix_is_fail_closed() {
        let cases = [
            (SignatureStatus::Signed, PackageSource::OfficialRepository, TrustDecision::Trusted),
            (SignatureStatus::Signed, PackageSource::SideLoad, TrustDecision::RequiresExplicitApproval),
            (SignatureStatus::ChecksumVerified, PackageSource::OfficialRepository, TrustDecision::RequiresExplicitApproval),
            (SignatureStatus::Unsigned, PackageSource::OfficialRepository, TrustDecision::Untrusted),
            (SignatureStatus::Unsigned, PackageSource::SideLoad, TrustDecision::Untrusted),
        ];
        for (sig, src, want) in cases {
            assert_eq!(evaluate_trust(sig, src), want, "{sig:?} from {src:?}");
        }
    }

    /// §35 via the manager: legal paths work, illegal paths are rejected —
    /// notably Broken → Enabled and Uninstalled → Enabled.
    #[test]
    fn lifecycle_manager_enforces_whitelist() {
        let mut m = LifecycleManager::default();
        assert_eq!(m.state("p"), PluginLifecycle::Uninstalled);
        m.apply("p", PluginLifecycle::Installed).unwrap();
        m.apply("p", PluginLifecycle::Enabled).unwrap();
        m.apply("p", PluginLifecycle::Broken).unwrap();
        assert!(m.apply("p", PluginLifecycle::Enabled).is_err(), "Broken never auto-Enabled");
        // repair path: Broken -> Installed -> Enabled
        m.apply("p", PluginLifecycle::Installed).unwrap();
        m.apply("p", PluginLifecycle::Enabled).unwrap();
        assert_eq!(m.state("p"), PluginLifecycle::Enabled);
        assert!(m.apply("p", PluginLifecycle::Installed).is_err());
    }
}
