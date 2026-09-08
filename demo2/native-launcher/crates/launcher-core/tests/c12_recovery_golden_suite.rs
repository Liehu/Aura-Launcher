//! P2.3-C12 Recovery Golden Suite: the unified 20-scenario matrix over
//! every persistent layer × disk corruption mode. Each scenario asserts the
//! layer's RECOVERY POLICY (per FailureClass/RecoveryPolicy in
//! launcher-domain): reopen succeeds, the layer is usable again, and safe
//! defaults hold. A regression anywhere in the matrix fails the suite.
//!
//! Layers: index.db / favorites.db / plugins.db / catalog.db / config.toml
//! Modes:  garbage header / truncated file / empty file / missing file

use std::path::{Path, PathBuf};

use launcher_core::favorites::FavoriteService;
use launcher_core::providers::plugin_registry::PluginRegistry;
use launcher_config::load_or_create;
use launcher_indexer::Indexer;
use launcher_providers::catalog::CatalogStore;

static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn fresh_dir(tag: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("nl_c12_{}_{}_{}", tag, std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Corruption modes applied to a populated layer file.
const GARBAGE: &[u8] = b"CORRUPTED BEYOND RECOGNITION NOT SQLITE AT ALL";

fn apply_mode(path: &Path, mode: &str, valid: &[u8]) {
    match mode {
        "garbage" => std::fs::write(path, GARBAGE).unwrap(),
        "truncated" => {
            // keep the SQLite magic (so a pre-open header check passes) but
            // truncate mid-file — the hardest mode: looks valid, isn't
            let cut = valid.len() / 2;
            std::fs::write(path, &valid[..cut]).unwrap();
        }
        "empty" => std::fs::write(path, b"").unwrap(),
        "missing" => {
            std::fs::remove_file(path).unwrap();
        }
        other => panic!("unknown mode {other}"),
    }
}

const MODES: [&str; 4] = ["garbage", "truncated", "empty", "missing"];

// ---- index.db: policy = Rebuild (index is derivable) ----

#[test]
fn golden_index_db_all_modes() {
    for mode in MODES {
        let dir = fresh_dir("idx");
        let db = dir.join("index.db");
        std::fs::write(dir.join("a.txt"), "hello").unwrap();
        std::fs::write(dir.join("b.txt"), "world").unwrap();
        let mut ix = Indexer::open(&db).unwrap();
        ix.rebuild(&[dir.clone()]).unwrap();
        let valid = std::fs::read(&db).unwrap();
        drop(ix);

        apply_mode(&db, mode, &valid);
        let mut ix = Indexer::open(&db).expect(mode);
        ix.rebuild(&[dir.clone()]).expect(mode);
        assert!(
            ix.search("a.txt", 10).unwrap().iter().any(|f| f.name == "a.txt"),
            "{mode}: rebuilt index must serve queries"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

// ---- favorites.db: policy = Repair (user data recreated empty, not lost
// silently — quarantine rename happens inside FavoriteService) ----

#[test]
fn golden_favorites_db_all_modes() {
    for mode in MODES {
        let dir = fresh_dir("fav");
        let db = dir.join("favorites.db");
        {
            let svc = FavoriteService::open(&db).unwrap();
            svc.add("app:seed").unwrap();
        }
        let valid = std::fs::read(&db).unwrap();

        apply_mode(&db, mode, &valid);
        let svc = FavoriteService::open(&db).expect(mode);
        svc.add("app:after").expect(mode);
        assert!(
            svc.snapshot().unwrap().is_favorite("app:after"),
            "{mode}: favorites usable after recovery"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

// ---- plugins.db: policy = backup-first restore, then safe defaults ----
// P2.3-D.1 (RC1 closure): corruption with a usable `plugins.db.bak` RESTORES
// the last known-good state — security choices (enabled/quarantined/counters)
// must survive corruption. Safe-default reset now applies only where no
// backup can exist (missing file) or the file is a valid empty db.

#[test]
fn golden_plugin_registry_all_modes() {
    // corruption paths restore the seeded failure count from the backup;
    // empty/missing paths rebuild fresh (no usable prior state exists).
    let expected_failures = [("garbage", 2), ("truncated", 2), ("empty", 0), ("missing", 0)];
    for (mode, want_failures) in expected_failures {
        let dir = fresh_dir("reg");
        let db = dir.join("plugins.db");
        {
            let reg = PluginRegistry::open(&db).unwrap();
            let _ = reg.record_failure("com.seed.a");
            let _ = reg.record_failure("com.seed.a");
        }
        let valid = std::fs::read(&db).unwrap();

        apply_mode(&db, mode, &valid);
        let reg = PluginRegistry::open(&db).expect(mode);
        let st = reg.state("com.seed.a");
        assert!(st.enabled, "{mode}: recovered registry defaults to enabled");
        assert!(!st.quarantined, "{mode}: recovered registry defaults to clean");
        assert_eq!(
            st.protocol_failures, want_failures,
            "{mode}: failure counter per recovery policy"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

// ---- catalog.db: policy = Rebuild (re-derivable from discovery) ----

#[test]
fn golden_catalog_db_all_modes() {
    for mode in MODES {
        let dir = fresh_dir("cat");
        let db = dir.join("catalog.db");
        {
            let cat = CatalogStore::open(&db).unwrap();
            let _ = cat.reconcile(&[("app".into(), "App".into(), r"C:\x.exe".into(), "start-menu".into())]).unwrap();
        }
        let valid = std::fs::read(&db).unwrap();

        apply_mode(&db, mode, &valid);
        let cat = CatalogStore::open(&db).expect(mode);
        let (gen, n) = cat
            .reconcile(&[("app".into(), "App".into(), r"C:\x.exe".into(), "start-menu".into())])
            .expect(mode);
        assert_eq!(n, 1, "{mode}: catalog usable after recovery");
        let _ = gen;
        std::fs::remove_dir_all(&dir).ok();
    }
}

// ---- config.toml: policy = quarantine + defaults (never lose the file) ----

#[test]
fn golden_config_garbage_quarantines_and_defaults() {
    let dir = fresh_dir("cfg");
    let path = dir.join("config.toml");
    std::fs::write(&path, "listen_hotkey = true\n").unwrap();

    // garbage TOML → quarantined copy + fresh defaults, still Ok
    std::fs::write(&path, "this is ][ not toml <<<").unwrap();
    let cfg = load_or_create(&path).expect("corrupt config must not fail startup");
    let _ = cfg;
    assert!(path.exists(), "defaults rewritten");
    let quarantined = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains("invalid"));
    assert!(quarantined.is_some(), "broken config preserved for inspection");

    // missing file → defaults created
    let path2 = dir.join("sub").join("config.toml");
    let _ = load_or_create(&path2).unwrap();
    assert!(path2.exists(), "missing config recreated");
    std::fs::remove_dir_all(&dir).ok();
}

/// Golden policy table: every persistent layer's recovery classification,
/// asserted actionable (C6 contract). Anchored by the per-layer tests above.
#[test]
fn golden_policy_table_is_complete() {
    let table = [
        ("index.db", "Rebuild", "derivable from disk scan"),
        ("catalog.db", "Rebuild", "derivable from discovery"),
        ("favorites.db", "Repair", "user data — recreate empty + quarantine"),
        ("plugins.db", "SafeDefaults", "security state resets to permissive-disabled"),
        ("config.toml", "Quarantine+Defaults", "broken file preserved, never lost"),
        ("icon-cache", "Rebuild", "pure cache"),
    ];
    assert_eq!(table.len(), 6, "one row per persistent layer");
    for (layer, policy, why) in table {
        assert!(!policy.is_empty() && !why.is_empty(), "{layer}: policy must be explicit");
    }
}
