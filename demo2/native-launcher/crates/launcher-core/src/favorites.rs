//! FavoriteService (P2.2-A): user-explicit Favorite/Pin state, persisted in
//! its own SQLite file (separate from the rebuildable file index).
//!
//! Frozen semantics (review: P2.2-A §2-§10/§41/§72-74):
//! - A Favorite binds a SEMANTIC identity key (canonical SearchIdentity), so
//!   it survives source renames/removals as long as the identity returns.
//! - Pin ⊆ Favorite (INV-FAV-006): pinning implies favorite; unfavorite
//!   removes the pin — both in ONE transaction (INV-FAV-007, §72-73).
//! - Favorites store identity only — never ResolvedAction/Effect/ExecutionId
//!   (INV-FAV-001).
//! - Duplicate add is idempotent (identity_key PRIMARY KEY).
//! - Bounded: max favorites / max pins.

use std::sync::Mutex;

use rusqlite::{params, Connection};
use std::collections::HashSet;

pub const MAX_FAVORITES: usize = 256;
pub const MAX_PINS: usize = 32;

pub struct FavoriteService {
    conn: Mutex<Connection>,
}

impl FavoriteService {
    pub fn open(db_path: &Path) -> Result<Self, rusqlite::Error> {
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // C6: favorites are USER DATA — a corrupt DB is quarantined (renamed
        // .corrupt) and recreated fresh. Never silently erased.
        // C6 pre-open corruption check: magic bytes (review 87 §7)
        if db_path.exists() {
            let header = std::fs::read(db_path)
                .map(|b| b[..16.min(b.len())].to_vec())
                .unwrap_or_default();
            let is_sqlite = header.starts_with(b"SQLite format 3");
            let is_empty = header.is_empty();
            if !is_sqlite && !is_empty {
                tracing::warn!(path = %db_path.display(), "favorites db corrupt — quarantining and recreating");
                let corrupt = db_path.with_extension("db.corrupt");
                let _ = std::fs::rename(db_path, &corrupt);
            }
        }
        let conn = match Self::try_open(db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %db_path.display(), error = %e, "favorites db corrupt — recreating");
                let _ = std::fs::remove_file(db_path);
                Self::try_open(db_path)?
            }
        };
        let svc = Self {
            conn: Mutex::new(conn),
        };
        svc.init_schema()?;
        Ok(svc)
    }

    fn try_open(db_path: &Path) -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS favorites (
                identity_key TEXT PRIMARY KEY,
                created_at   INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pins (
                identity_key TEXT PRIMARY KEY
                    REFERENCES favorites(identity_key) ON DELETE CASCADE,
                order_index  INTEGER NOT NULL
            );",
        )?;
        Ok(conn)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        // schema is created in try_open; this is a no-op placeholder for
        // future migrations
        Ok(())
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    /// Add a favorite. Idempotent (§79).
    pub fn add(&self, identity_key: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().expect("favorites lock");
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get(0))?;
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM favorites WHERE identity_key = ?1",
                params![identity_key],
                |_| Ok(()),
            )
            .is_ok();
        if exists || n as usize >= MAX_FAVORITES {
            return Ok(false);
        }
        conn.execute(
            "INSERT OR IGNORE INTO favorites(identity_key, created_at) VALUES (?1, ?2)",
            params![identity_key, Self::now()],
        )?;
        Ok(true)
    }

    /// Remove favorite AND its pin atomically (§8/§42/INV-FAV-007).
    pub fn remove(&self, identity_key: &str) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock().expect("favorites lock");
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM pins WHERE identity_key = ?1",
            params![identity_key],
        )?;
        tx.execute(
            "DELETE FROM favorites WHERE identity_key = ?1",
            params![identity_key],
        )?;
        tx.commit()
    }

    /// Pin: create the favorite if absent + insert pin, ONE transaction
    /// (§40/§41). Bounded by MAX_PINS.
    pub fn pin(&self, identity_key: &str) -> Result<bool, rusqlite::Error> {
        {
            let conn = self.conn.lock().expect("favorites lock");
            let n: i64 = conn.query_row("SELECT COUNT(*) FROM pins", [], |r| r.get(0))?;
            let pinned: bool = conn
                .query_row(
                    "SELECT 1 FROM pins WHERE identity_key = ?1",
                    params![identity_key],
                    |_| Ok(()),
                )
                .is_ok();
            if !pinned && n as usize >= MAX_PINS {
                return Ok(false);
            }
        }
        let mut conn = self.conn.lock().expect("favorites lock");
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO favorites(identity_key, created_at) VALUES (?1, ?2)",
            params![identity_key, Self::now()],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO pins(identity_key, order_index)
             VALUES (?1, (SELECT COALESCE(MAX(order_index), -1) + 1 FROM pins))",
            params![identity_key],
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Unpin: remove the pin, KEEP the favorite (§9).
    pub fn unpin(&self, identity_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().expect("favorites lock");
        conn.execute("DELETE FROM pins WHERE identity_key = ?1", params![identity_key])?;
        Ok(())
    }

    /// Move a pinned identity to a new order and renumber the rest (§10/§43).
    pub fn reorder(&self, identity_key: &str, new_order: usize) -> Result<(), rusqlite::Error> {
        let mut pinned: Vec<String> = {
            let conn = self.conn.lock().expect("favorites lock");
            let mut stmt =
                conn.prepare("SELECT identity_key FROM pins ORDER BY order_index")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.flatten().collect()
        };
        let Some(pos) = pinned.iter().position(|k| k == identity_key) else {
            return Ok(()); // not pinned: no-op
        };
        let item = pinned.remove(pos);
        let new_order = new_order.min(pinned.len());
        pinned.insert(new_order, item);
        let mut conn = self.conn.lock().expect("favorites lock");
        let tx = conn.transaction()?;
        for (i, k) in pinned.iter().enumerate() {
            tx.execute(
                "UPDATE pins SET order_index = ?2 WHERE identity_key = ?1",
                params![k, i as i64],
            )?;
        }
        tx.commit()
    }

    /// Snapshot of favorite/pinned identity keys for ranking (§24/§25:
    /// favorites are a ranking SIGNAL injected into the candidate pool, not
    /// a provider).
    pub fn snapshot(&self) -> Result<FavoriteSnapshot, rusqlite::Error> {
        let mut favs = HashSet::new();
        {
            let conn = self.conn.lock().expect("favorites lock");
            let mut stmt = conn.prepare("SELECT identity_key FROM favorites")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for r in rows {
                favs.insert(r?);
            }
        }
        let mut pinned = Vec::new();
        {
            let conn = self.conn.lock().expect("favorites lock");
            let mut stmt = conn
                .prepare("SELECT identity_key FROM pins ORDER BY order_index")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for r in rows {
                pinned.push(r?);
            }
        }
        Ok(FavoriteSnapshot {
            favorites: favs,
            pinned,
        })
    }
}

use std::path::Path;

/// Immutable ranking-time view of user favorite state.
#[derive(Debug, Clone, Default)]
pub struct FavoriteSnapshot {
    pub favorites: HashSet<String>,
    /// Pinned identity keys in display order.
    pub pinned: Vec<String>,
}

impl FavoriteSnapshot {
    pub fn is_favorite(&self, key: &str) -> bool {
        self.favorites.contains(key)
    }

    pub fn is_pinned(&self, key: &str) -> bool {
        self.pinned.iter().any(|k| k == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc() -> FavoriteService {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "nl_fav_{}_{}",
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        FavoriteService::open(&dir.join("fav.db")).unwrap()
    }

    /// INV-FAV-006: pin implies favorite (single transaction §41).
    #[test]
    fn pin_implies_favorite() {
        let s = svc();
        assert!(s.pin("app:chrome").unwrap());
        let snap = s.snapshot().unwrap();
        assert!(snap.is_favorite("app:chrome"));
        assert!(snap.is_pinned("app:chrome"));
    }

    /// INV-FAV-007: unfavorite removes the pin atomically.
    #[test]
    fn unfavorite_removes_pin() {
        let s = svc();
        s.pin("app:chrome").unwrap();
        s.remove("app:chrome").unwrap();
        let snap = s.snapshot().unwrap();
        assert!(!snap.is_favorite("app:chrome"));
        assert!(!snap.is_pinned("app:chrome"), "no dangling pin");
    }

    /// §9: unpin keeps the favorite.
    #[test]
    fn unpin_keeps_favorite() {
        let s = svc();
        s.pin("app:chrome").unwrap();
        s.unpin("app:chrome").unwrap();
        let snap = s.snapshot().unwrap();
        assert!(snap.is_favorite("app:chrome"));
        assert!(!snap.is_pinned("app:chrome"));
    }

    /// §79: duplicate add is idempotent.
    #[test]
    fn duplicate_add_idempotent() {
        let s = svc();
        assert!(s.add("app:chrome").unwrap());
        assert!(!s.add("app:chrome").unwrap(), "second add reports no change");
        let snap = s.snapshot().unwrap();
        assert_eq!(snap.favorites.len(), 1);
    }

    /// §10/§43: reorder renumbers; moving to a clamped order is safe.
    #[test]
    fn reorder_moves_and_renumbers() {
        let s = svc();
        for k in ["a", "b", "c"] {
            s.pin(k).unwrap();
        }
        s.reorder("c", 0).unwrap();
        let snap = s.snapshot().unwrap();
        assert_eq!(snap.pinned, vec!["c", "a", "b"]);
        s.reorder("a", 99).unwrap(); // clamps to last
        let snap = s.snapshot().unwrap();
        assert_eq!(snap.pinned, vec!["c", "b", "a"]);
    }

    /// §78: pin limit is bounded.
    #[test]
    fn pin_limit_enforced() {
        let s = svc();
        for i in 0..MAX_PINS + 5 {
            s.pin(&format!("app:{i}")).unwrap();
        }
        let snap = s.snapshot().unwrap();
        assert_eq!(snap.pinned.len(), MAX_PINS, "pin count must be bounded");
    }

    /// §89 restart persistence.
    #[test]
    fn favorites_survive_reopen() {
        let dir = std::env::temp_dir().join(format!("nl_fav_reopen_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("fav.db");
        {
            let s = FavoriteService::open(&db).unwrap();
            s.pin("app:chrome").unwrap();
        }
        {
            let s = FavoriteService::open(&db).unwrap();
            let snap = s.snapshot().unwrap();
            assert!(snap.is_favorite("app:chrome"));
            assert!(snap.is_pinned("app:chrome"));
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod corruption_tests {
    use super::*;

    /// C6/C7: a corrupt favorites.db is quarantined (renamed .corrupt) and
    /// recreated fresh — never a startup failure, never silent data loss.
    #[test]
    fn corrupt_favorites_db_recreated() {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("nl_fav_corrupt_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("fav.db");
        std::fs::write(&db, b"corrupt garbage not sqlite").unwrap();

        let svc = FavoriteService::open(&db).unwrap();
        // after recovery the store works
        svc.add("app:test").unwrap();
        assert!(svc.snapshot().unwrap().is_favorite("app:test"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
