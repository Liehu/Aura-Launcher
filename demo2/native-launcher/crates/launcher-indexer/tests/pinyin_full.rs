//! Full-pinyin search integration (the deferred "full pinyin table"
//! optimization): the FTS index carries a `pinyin_full` column populated
//! from the accurate common-character table, and queries typed as full
//! pinyin (e.g. "wenjian") match Chinese filenames.
#![cfg(windows)]

use launcher_domain::{FileObservation, VerifiedChange};
use launcher_indexer::Indexer;
use std::path::PathBuf;

fn obs(path: &str, name: &str) -> VerifiedChange {
    VerifiedChange::Upsert(FileObservation {
        path: path.into(),
        normalized_path: path.to_lowercase(),
        is_dir: false,
        size: 1,
        modified_ms: 0,
    })
}

/// Full pinyin typed by the user matches Chinese-named files.
#[test]
fn full_pinyin_query_matches() {
    let mut ix = Indexer::in_memory().unwrap();
    let stats = ix
        .apply_batch(&[
            obs("C:/docs/文件夹/报告.txt", "报告.txt"),
            obs("C:/docs/文件夹/说明.md", "说明.md"),
            obs("C:/docs/plain.txt", "plain.txt"),
        ])
        .unwrap();
    assert_eq!(stats.upserted, 3);

    let hits = ix.search("baogao", 10).unwrap();
    assert!(
        hits.iter().any(|f| f.name == "报告.txt"),
        "full pinyin must match: {:?}",
        hits.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    // first-letter queries keep working on the same index
    let hits = ix.search("sm", 10).unwrap();
    assert!(hits.iter().any(|f| f.name == "说明.md"));
    // english names unaffected
    let hits = ix.search("plain", 10).unwrap();
    assert!(hits.iter().any(|f| f.name == "plain.txt"));
}

/// An OLD-shape index (files_fts without pinyin_full) is migrated on open:
/// the table is dropped, recreated with the new column, and repopulated.
#[test]
fn old_schema_is_migrated() {
    let dir = std::env::temp_dir().join(format!("nl-pinyin-mig-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let db = dir.join("index.db");

    // 1. create an old-shape FTS table (pre-pinyin_full schema)
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE files (id INTEGER PRIMARY KEY, path TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL, parent TEXT NOT NULL, is_dir INTEGER NOT NULL,
                size INTEGER NOT NULL, modified_ms INTEGER NOT NULL);
            CREATE VIRTUAL TABLE files_fts USING fts5(name, pinyin_init, path UNINDEXED);
            "#,
        )
        .unwrap();
    }
    // 2. open with the current schema — migration must not fail
    let mut ix = Indexer::open(&db).unwrap();
    ix.apply_batch(&[obs("C:/docs/报告.txt", "报告.txt")]).unwrap();
    let hits = ix.search("baogao", 10).unwrap();
    assert!(hits.iter().any(|f| f.name == "报告.txt"), "migrated index serves full pinyin");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = PathBuf::new();
}