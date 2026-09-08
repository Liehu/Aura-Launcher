//! P2.3-C Recovery Golden Suite (C12, review 87 §14): launcher-indexer
//! reliability scenarios — index corruption, generation monotonicity,
//! and rebuild recovery. Plugin/Favorites recovery are in their own crates.

use launcher_indexer::Indexer;

static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn setup(tag: &str) -> std::path::PathBuf {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("nl_rec_{tag}_{}_{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// REC-014: index corruption → delete + recreate (rebuildable).
#[test]
fn rec_014_index_corruption_rebuild() {
    let dir = setup("rec014");
    let db = dir.join("index.db");
    std::fs::write(&db, b"corrupt").unwrap();
    let mut ix = Indexer::open(&db).unwrap();
    assert_eq!(ix.status().unwrap().0, 0, "recovered empty");
    std::fs::write(dir.join("f.txt"), "x").unwrap();
    ix.rebuild(&[dir.clone()]).unwrap();
    assert!(ix.search("f.txt", 1).unwrap().len() > 0);
    std::fs::remove_dir_all(&dir).ok();
}

/// INV-RECOVERY-002: index generation is monotonically increasing across
/// "crash" (process restart) — it persists in the DB and never decreases.
#[test]
fn index_generation_monotonic_across_restart() {
    let dir = setup("rec_gen");
    let db = dir.join("index.db");
    std::fs::write(dir.join("f.txt"), "x").unwrap();

    let mut ix1 = Indexer::open(&db).unwrap();
    ix1.rebuild(&[dir.clone()]).unwrap();
    let g1 = ix1.generation().unwrap();
    drop(ix1);

    // "crash" and reopen — generation persists and only increases
    let mut ix2 = Indexer::open(&db).unwrap();
    ix2.rebuild(&[dir.clone()]).unwrap();
    let g2 = ix2.generation().unwrap();
    assert!(g2 > g1, "generation must be monotonically increasing");
    std::fs::remove_dir_all(&dir).ok();
}

/// REC-011: index writer failure (corrupt DB) → recovery via delete +
/// recreate; the indexer remains functional.
#[test]
fn index_writer_failure_recovery() {
    let dir = setup("rec011");
    let db = dir.join("index.db");
    std::fs::write(&db, b"garbage bytes that are not sqlite").unwrap();
    let mut ix = Indexer::open(&db).unwrap();
    // after corruption recovery the index works
    std::fs::write(dir.join("f.txt"), "x").unwrap();
    ix.rebuild(&[dir.clone()]).unwrap();
    assert!(ix.search("f.txt", 1).unwrap().len() > 0);
    std::fs::remove_dir_all(&dir).ok();
}
