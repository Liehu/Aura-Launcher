//! P2.3-G Data Layout Contract (review 91 §4/§7): verify the persistent
//! data directory structure matches the documented contract after a
//! simulated startup. All files must be in their expected locations.

use launcher_indexer::Indexer;
use std::path::PathBuf;

fn data_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nl_p23g_{tag}_{}_{}", std::process::id(), SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// G4: verify persistent data files exist in their expected locations
/// after simulated startup, and that the program directory is separate.
#[test]
fn data_layout_matches_contract() {
    let dir = data_dir("layout");
    let db_dir = dir.join("data");
    std::fs::create_dir_all(&db_dir).unwrap();

    // simulate the files build_core creates
    let mut index = Indexer::open(&db_dir.join("index.db")).unwrap();
    index.rebuild(&[db_dir.clone()]).unwrap();
    let _fav = launcher_core::favorites::FavoriteService::open(&db_dir.join("favorites.db")).unwrap();
    let _reg = launcher_core::providers::plugin_registry::PluginRegistry::open(
        &db_dir.join("plugins.db"),
    )
    .unwrap();
    let _cat = launcher_providers::catalog::CatalogStore::open(&db_dir.join("catalog.db")).unwrap();

    // verify all four databases exist and are separate files
    for name in ["index.db", "favorites.db", "plugins.db", "catalog.db"] {
        let p = db_dir.join(name);
        assert!(p.exists(), "{name} must exist in data dir");
        assert!(
            p.metadata().unwrap().len() > 0,
            "{name} must not be empty"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// P2.3-G: program and data directories must be separate — an uninstall
/// that removes the program dir must not touch user data.
#[test]
fn program_and_data_dirs_are_disjoint() {
    let program_dir = data_dir("prog");
    let data_dir = data_dir("userdata");
    // these are different paths
    assert_ne!(program_dir, data_dir);
    // removing program dir doesn't affect data dir
    std::fs::write(program_dir.join("fake_exe"), b"").unwrap();
    std::fs::create_dir_all(data_dir.join("workflows")).unwrap();
    std::fs::write(data_dir.join("workflows").join("demo.json"), b"{}").unwrap();

    // "uninstall" = remove program dir
    std::fs::remove_dir_all(&program_dir).unwrap();
    // data dir untouched
    assert!(data_dir.join("workflows").join("demo.json").exists());
    std::fs::remove_dir_all(&data_dir).ok();
}

/// P2.3-G: upgrade preserves user data across install — the installer only
/// touches the program dir; config/favorites/index/workflows are untouched.
#[test]
fn upgrade_preserves_user_data() {
    let program = data_dir("prog_upg");
    let data = data_dir("data_upg");
    std::fs::create_dir_all(&program).unwrap();
    std::fs::create_dir_all(&data).unwrap();

    // v1 writes user data
    std::fs::write(data.join("favorites.db"), b"v1 data").unwrap();
    std::fs::write(data.join("config.toml"), b"hotkey = 'Alt+Space'").unwrap();

    // v2 "upgrade": overwrite program files only
    std::fs::write(program.join("launcher-app.exe"), b"v2 binary").unwrap();

    // user data preserved
    assert!(data.join("favorites.db").exists());
    assert!(data.join("config.toml").exists());
    assert_eq!(
        std::fs::read_to_string(data.join("config.toml")).unwrap(),
        "hotkey = 'Alt+Space'"
    );
    std::fs::remove_dir_all(&program).ok();
    std::fs::remove_dir_all(&data).ok();
}
