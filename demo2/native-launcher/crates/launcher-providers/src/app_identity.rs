//! Application Identity v1 (P2.4-A01, spec `01-P2.4-DESIGN-SPEC.md` §4.2/§4.3).
//!
//! [`ApplicationObservation`] is untrusted discovery output; [`canonical_identity`]
//! collapses observations from any number of sources into one stable identity
//! string with a fixed precedence. Identity resolution is PURE (spec §4.3 /
//! INV-IDENTITY-003): it reads no disk, performs no IO and has no side effects —
//! it normalizes and picks. An identity is a *key*, never execution authority.

use launcher_domain::normalize_path_identity;

/// One discovery observation, before normalization/merge (spec §4.2).
/// Every field is untrusted data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApplicationObservation {
    /// Stable id of the producing source (e.g. "start-menu", "uninstall",
    /// "packaged", "app-paths", "portable:<root>").
    pub source_id: String,
    /// Source kind tag (same vocabulary as source_id kinds above).
    pub source_kind: String,
    /// Source-scoped hint (e.g. lnk path, registry key) for the fallback tier.
    pub identity_hint: String,
    pub display_name: String,
    /// Canonical launch target if the observation has one (lnk target,
    /// uninstall command, portable exe).
    pub launch_target: Option<String>,
    /// Packaged identity (Package Family Name / MSIX identity) when present.
    pub package_identity: Option<String>,
    /// AUMID when present.
    pub aumid: Option<String>,
    /// App Paths registry resolution when present.
    pub app_path: Option<String>,
    /// Unix millis when observed (provenance only; never part of identity).
    pub observed_at: i64,
}

/// Canonical identity precedence (spec §4.3):
/// 1. packaged identity / AUMID      → `pkg:<…>` / `aumid:<…>`
/// 2. canonical Win32 exe target     → `win32:<normalized path>`
/// 3. canonical App Paths target     → `apppath:<normalized path>`
/// 4. portable exe target            → `win32:<normalized path>` (same tier as 2:
///    a portable exe IS a Win32 exe; portability affects discovery, not identity)
/// 5. source-scoped fallback         → `src:<source_kind>:<identity_hint>`
///
/// The fallback tier embeds the source scope so unrelated apps can never
/// collide; higher tiers are globally scoped by their namespace.
pub fn canonical_identity(obs: &ApplicationObservation) -> String {
    if let Some(pkg) = non_empty(&obs.package_identity) {
        return format!("pkg:{}", pkg.to_lowercase());
    }
    if let Some(aumid) = non_empty(&obs.aumid) {
        return format!("aumid:{}", aumid.to_lowercase());
    }
    if let Some(target) = non_empty(&obs.launch_target) {
        if is_exe(target) {
            return format!("win32:{}", normalize_path_identity(target));
        }
    }
    if let Some(app_path) = non_empty(&obs.app_path) {
        return format!("apppath:{}", normalize_path_identity(app_path));
    }
    format!("src:{}:{}", obs.source_kind, obs.identity_hint)
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn is_exe(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("lnk"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(source: &str, target: Option<&str>) -> ApplicationObservation {
        ApplicationObservation {
            source_id: source.into(),
            source_kind: source.into(),
            identity_hint: format!("{source}:hint"),
            display_name: "App".into(),
            launch_target: target.map(String::from),
            observed_at: 1_790_000_000_000,
            ..Default::default()
        }
    }

    /// Required case: same exe from different sources → same identity.
    #[test]
    fn same_exe_different_sources_same_identity() {
        let a = canonical_identity(&obs("start-menu", Some(r"C:\Tools\App.exe")));
        let b = canonical_identity(&obs("uninstall", Some(r"c:\tools\app.EXE")));
        assert_eq!(a, b, "exe identity normalizes case and collapses sources");
        assert!(a.starts_with("win32:"));
    }

    /// Required case: different exes → different identities.
    #[test]
    fn different_exes_different_identities() {
        let a = canonical_identity(&obs("start-menu", Some(r"C:\A\App.exe")));
        let b = canonical_identity(&obs("start-menu", Some(r"C:\B\App.exe")));
        assert_ne!(a, b);
    }

    /// Required case: packaged identity is stable and wins over exe target.
    #[test]
    fn packaged_identity_takes_precedence() {
        let mut o = obs("packaged", Some(r"C:\Apps\Store\App.exe"));
        o.package_identity = Some("Aura.App_abc123".into());
        let id = canonical_identity(&o);
        assert_eq!(id, "pkg:aura.app_abc123");
        // case of the package id must not change identity
        o.package_identity = Some("AURA.APP_ABC123".into());
        assert_eq!(canonical_identity(&o), id);
    }

    /// Required case: portable identity — no package, direct exe target.
    #[test]
    fn portable_identity_uses_win32_tier() {
        let id = canonical_identity(&obs("portable:c:\\tools", Some(r"C:\Tools\my-tool.exe")));
        assert!(id.starts_with("win32:"), "portable exe is a win32 identity");
    }

    /// Fallback identity cannot collide across unrelated apps (source-scoped).
    #[test]
    fn fallback_identity_is_source_scoped() {
        let mut a = obs("start-menu", None);
        a.identity_hint = "shell:AppsFolder\\X".into();
        let mut b = obs("uninstall", None);
        b.identity_hint = "shell:AppsFolder\\X".into();
        assert_ne!(canonical_identity(&a), canonical_identity(&b));
        assert!(canonical_identity(&a).starts_with("src:start-menu:"));
    }

    /// AUMID outranks exe target (precedence 1 > 2).
    #[test]
    fn aumid_outranks_exe_target() {
        let mut o = obs("packaged", Some(r"C:\Apps\App.exe"));
        o.aumid = Some("Aura.App!App".into());
        assert_eq!(canonical_identity(&o), "aumid:aura.app!app");
    }

    /// Identity resolution is pure: identical inputs give identical outputs
    /// (deterministic, no IO to observe — this pins the contract).
    #[test]
    fn identity_is_deterministic() {
        let o = obs("start-menu", Some(r"C:\Program Files\App\App.exe"));
        assert_eq!(canonical_identity(&o), canonical_identity(&o));
    }
}
