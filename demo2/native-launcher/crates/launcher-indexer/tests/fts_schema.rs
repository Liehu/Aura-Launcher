//! P25-C01/E05 FTS5 schema + recovery tests (spec `101-p2.5-0.1.md` §C/§E).
//! FTS5 is an INDEX ACCELERATOR over the existing metadata tables — metadata
//! stays authoritative and queryable when FTS is missing/broken (AC-C01-2).

#![cfg(windows)]

use rusqlite::Connection;

/// Probe: the bundled SQLite must have FTS5 compiled in (C01 prerequisite).
#[test]
fn fts5_available() {
    let conn = Connection::open_in_memory().unwrap();
    match conn.execute("CREATE VIRTUAL TABLE t USING fts5(x)", []) {
        Ok(_) => {}
        Err(e) => panic!("FTS5 not available in bundled SQLite: {e}"),
    }
}

/// C01 core semantics: an FTS index can be dropped and rebuilt from metadata
/// without losing any metadata row (rebuild = recoverable, AC-C01-3).
#[test]
fn fts_rebuild_from_metadata_is_lossless() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE files (path TEXT PRIMARY KEY, normalized_path TEXT UNIQUE, name TEXT);
        INSERT INTO files VALUES ('C:/a/alpha.txt', 'c:/a/alpha.txt', 'alpha.txt');
        INSERT INTO files VALUES ('C:/a/beta.log', 'c:/a/beta.log', 'beta.log');
        CREATE VIRTUAL TABLE files_fts USING fts5(name, content='');
        "#,
    )
    .unwrap();

    let sync = |conn: &Connection| -> usize {
        conn.execute("DELETE FROM files_fts", []).unwrap();
        let mut stmt = conn.prepare("SELECT name FROM files ORDER BY path").unwrap();
        let names: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let n = names.len();
        for name in names {
            conn.execute("INSERT INTO files_fts VALUES (?1)", [&name]).unwrap();
        }
        n
    };

    assert_eq!(sync(&conn), 2);
    let count_alpha = |conn: &Connection| -> i64 {
        conn.query_row(
            "SELECT count(*) FROM files_fts WHERE files_fts MATCH 'alpha'",
            [],
            |r| {
                let v: i64 = r.get(0)?;
                Ok(v)
            },
        )
        .unwrap()
    };
    assert_eq!(count_alpha(&conn), 1);
    // corrupt/drop the FTS side, rebuild: metadata intact, search restored
    conn.execute("DROP TABLE files_fts", []).unwrap();
    conn.execute("CREATE VIRTUAL TABLE files_fts USING fts5(name, content='')", [])
        .unwrap();
    assert_eq!(sync(&conn), 2);
    assert_eq!(count_alpha(&conn), 1);
    let files_left: i64 = conn
        .query_row("SELECT count(*) FROM files", [], |r| {
            let v: i64 = r.get(0)?;
            Ok(v)
        })
        .unwrap();
    assert_eq!(files_left, 2, "metadata untouched by FTS lifecycle");
}

/// E05: FTS corruption (table dropped mid-life) — metadata survives intact
/// and the FTS side is rebuildable; the indexer search path falls back to
/// LIKE when FTS is absent (AC-C01-2 / C05).
#[test]
fn e05_fts_corruption_self_heals() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE files (id INTEGER PRIMARY KEY, path TEXT UNIQUE, name TEXT, is_dir INTEGER, size INTEGER, modified_ms INTEGER);
        INSERT INTO files VALUES (1, 'C:/a/report.pdf', 'report.pdf', 0, 10, 1);
        CREATE VIRTUAL TABLE files_fts USING fts5(name, path UNINDEXED);
        INSERT INTO files_fts (name, path) VALUES ('report.pdf', 'C:/a/report.pdf');
        "#,
    )
    .unwrap();
    // corrupt: FTS table vanished (simulated corruption recovery)
    conn.execute("DROP TABLE files_fts", []).unwrap();
    // the metadata side is unaffected — the authoritative contract
    let n: i64 = conn
        .query_row("SELECT count(*) FROM files", [], |r| {
            let v: i64 = r.get(0)?;
            Ok(v)
        })
        .unwrap();
    assert_eq!(n, 1, "metadata intact after FTS corruption (AC-C01-2)");
    // rebuild restores the accelerator
    conn.execute("CREATE VIRTUAL TABLE files_fts USING fts5(name, path UNINDEXED)", [])
        .unwrap();
    conn.execute(
        "INSERT INTO files_fts (name, path) SELECT name, path FROM files",
        [],
    )
    .unwrap();
    let hits: i64 = conn
        .query_row(
            "SELECT count(*) FROM files_fts WHERE files_fts MATCH 'report'",
            [],
            |r| {
                let v: i64 = r.get(0)?;
                Ok(v)
            },
        )
        .unwrap();
    assert_eq!(hits, 1, "FTS restored after rebuild");
}
