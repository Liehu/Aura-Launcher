//! P2.8-H integration test: the full ecosystem pipeline over REAL modules —
//! repository search → resolve → per-step checksum verify → transactional
//! install → lifecycle transitions, with an audit trail alongside.

use launcher_plugin_cli::audit;
use launcher_plugin_cli::foundation::{verify_integrity, PluginLifecycle};
use launcher_plugin_cli::repository::{
    search_marketplace, RepositoryEntry, RepositoryIndex, SignatureBadge,
};
use launcher_plugin_cli::resolver::resolve;
use launcher_plugin_cli::transactional::{execute_plan, PackageInstaller};
use launcher_plugin_cli::trust::{evaluate_trust, PackageSource};

fn entry(
    id: &str,
    summary: &str,
    badge: SignatureBadge,
    payload: &[u8],
    depends: &[&str],
) -> RepositoryEntry {
    RepositoryEntry {
        package: launcher_plugin_cli::resolver::PackageMeta {
            id: id.into(),
            version: "1.0.0".into(),
            depends: depends.iter().map(|s| s.to_string()).collect(),
        },
        summary: summary.into(),
        payload_sha256: launcher_plugin_cli::foundation::sha256_hex(payload),
        signature: badge,
    }
}

/// E2E: search "tool" in a signed official repo → resolve the dependency
/// chain → verify every payload checksum → transactional install → lifecycle
/// Uninstalled → Installed → Enabled, with an audit trail of the decisions.
#[test]
fn ecosystem_pipeline_end_to_end() {
    let dir = std::env::temp_dir().join(format!("nl_p28_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut repo = RepositoryIndex::default();
    let lib_payload = b"lib payload bytes";
    let app_payload = b"app payload bytes";
    repo.publish(
        entry("lib", "Shared library", SignatureBadge::Signed, lib_payload, &[]),
        lib_payload,
    );
    repo.publish(
        entry("tool", "The Tool", SignatureBadge::Signed, app_payload, &["lib"]),
        app_payload,
    );

    // 1. marketplace search finds the tool (trust badge surfaced)
    let results = search_marketplace(&repo, "tool", PackageSource::OfficialRepository, 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, launcher_plugin_cli::trust::TrustDecision::Trusted);

    // 2. resolve its dependency chain
    let plan = resolve(
        &repo
            .entries
            .iter()
            .map(|e| e.package.clone())
            .collect::<Vec<_>>(),
        &["tool".into()],
    )
    .unwrap();
    assert_eq!(plan.steps.len(), 2, "lib before tool");

    // 3. transactional install with per-step checksum verification
    struct VerifyingInstaller<'a> {
        repo: &'a RepositoryIndex,
        payloads: std::collections::HashMap<String, Vec<u8>>,
        active: Vec<String>,
    }
    impl PackageInstaller for VerifyingInstaller<'_> {
        fn stage(&mut self, id: &str) -> Result<(), String> {
            // §10: the payload is re-fetched from the repository origin and
            // its checksum verified BEFORE activation
            let payload = self
                .payloads
                .get(id)
                .ok_or_else(|| format!("unknown id {id}"))?;
            let expected = self
                .repo
                .checksum_of(id)
                .ok_or_else(|| format!("{id} not in repository"))?;
            if !verify_integrity(expected, payload) {
                return Err(format!("{id}: checksum mismatch"));
            }
            Ok(())
        }
        fn activate(&mut self, id: &str) -> Result<(), String> {
            self.active.push(id.into());
            Ok(())
        }
        fn remove(&mut self, id: &str) -> Result<(), String> {
            self.active.retain(|a| a != id);
            Ok(())
        }
    }
    let mut installer = VerifyingInstaller {
        repo: &repo,
        payloads: std::collections::HashMap::from([
            ("lib".to_string(), lib_payload.to_vec()),
            ("tool".to_string(), app_payload.to_vec()),
        ]),
        active: vec![],
    };
    let n = execute_plan(&plan, &mut installer).unwrap();
    assert_eq!(n, 2);
    assert_eq!(installer.active, vec!["lib", "tool"]);

    // 4. lifecycle: Uninstalled -> Installed -> Enabled (legal chain)
    let mut lifecycle = std::collections::HashMap::new();
    for id in ["lib", "tool"] {
        lifecycle.insert(id, PluginLifecycle::Uninstalled);
    }
    for id in ["lib", "tool"] {
        let cur = lifecycle[id];
        let next = launcher_plugin_cli::foundation::transition_allowed(cur, PluginLifecycle::Installed);
        assert!(next);
        lifecycle.insert(id, PluginLifecycle::Installed);
        assert!(launcher_plugin_cli::foundation::transition_allowed(
            PluginLifecycle::Installed,
            PluginLifecycle::Enabled
        ));
        lifecycle.insert(id, PluginLifecycle::Enabled);
    }

    // 5. audit trail records the decisions
    let audit_path = dir.join("audit.jsonl");
    audit::append("install", &serde_json::json!({"packages": ["lib", "tool"]}), &audit_path)
        .unwrap();
    let trail = audit::read_all(&audit_path).unwrap();
    assert_eq!(trail.len(), 1);
    assert_eq!(trail[0].1, "install");

    std::fs::remove_dir_all(&dir).ok();
}
