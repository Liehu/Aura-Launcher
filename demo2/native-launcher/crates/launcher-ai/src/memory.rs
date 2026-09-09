//! Agent Memory (P27-E01–E04, spec `P2.7 开发设计规范` §26): the three
//! sanctioned memory classes — Session Memory, Run Memory, and optional
//! User Preferences. No unbounded long-term memory exists (§26 red line).
//!
//! Every class is bounded (FIFO eviction at the caps), explicit (entries
//! are plain data the host can render), user-visible (listing APIs), and
//! deletable (per-key and clear-all, E04). Scoped: session memory is keyed
//! by session id, run memory by run id — nothing crosses scopes.

/// Default caps (§26 bounded): entries per scope and chars per entry.
pub const MAX_SESSIONS: usize = 8;
pub const MAX_RUNS_PER_SESSION: usize = 20;
pub const MAX_PREFS: usize = 32;
pub const MAX_ENTRY_CHARS: usize = 512;

fn bounded(text: &str) -> String {
    let mut owned: String = text.chars().take(MAX_ENTRY_CHARS).collect();
    // strip control characters (except space/newline) so memory can never
    // smuggle terminal/protocol noise back into a prompt
    owned.retain(|c| c == '\n' || c == '\r' || c == '\t' || !c.is_control());
    owned
}

/// One memory entry: who wrote it and what (plain renderable data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    pub kind: &'static str,
    pub text: String,
    pub at_ms: i64,
}

/// E01: session memory — the goals and outcomes of an agent session,
/// bounded per session and in total.
#[derive(Debug, Default)]
pub struct SessionMemory {
    sessions: Vec<(String, Vec<MemoryEntry>)>,
}

impl SessionMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one entry to a session; evicts the OLDEST SESSION when the
    /// total session cap is hit (scoped forgetting, never cross-session).
    pub fn push(&mut self, session_id: &str, kind: &'static str, text: &str, at_ms: i64) {
        let entry = MemoryEntry { kind, text: bounded(text), at_ms };
        if let Some((_, entries)) = self.sessions.iter_mut().find(|(id, _)| id == session_id) {
            entries.push(entry);
            if entries.len() > MAX_RUNS_PER_SESSION {
                entries.remove(0);
            }
            return;
        }
        self.sessions.push((session_id.to_string(), vec![entry]));
        if self.sessions.len() > MAX_SESSIONS {
            self.sessions.remove(0);
        }
    }

    pub fn entries(&self, session_id: &str) -> &[MemoryEntry] {
        self.sessions
            .iter()
            .find(|(id, _)| id == session_id)
            .map(|(_, e)| e.as_slice())
            .unwrap_or(&[])
    }

    /// E04: delete one session's memory (user-visible deletion).
    pub fn delete_session(&mut self, session_id: &str) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|(id, _)| id != session_id);
        self.sessions.len() != before
    }

    /// E04: clear everything.
    pub fn clear(&mut self) {
        self.sessions.clear();
    }

    pub fn session_ids(&self) -> Vec<String> {
        self.sessions.iter().map(|(id, _)| id.clone()).collect()
    }
}

/// E02: run memory — one record per agent run inside a session (goal and
/// final status), bounded FIFO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub run_id: String,
    pub goal: String,
    pub status: String,
    pub turns: u64,
    pub at_ms: i64,
}

#[derive(Debug, Default)]
pub struct RunMemory {
    runs: Vec<RunRecord>,
}

impl RunMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, run: RunRecord) {
        let mut run = run;
        run.goal = bounded(&run.goal);
        if self.runs.len() >= MAX_RUNS_PER_SESSION {
            self.runs.remove(0);
        }
        self.runs.push(run);
    }

    pub fn runs(&self) -> &[RunRecord] {
        &self.runs
    }

    /// E04: forget one run / all runs.
    pub fn delete_run(&mut self, run_id: &str) -> bool {
        let before = self.runs.len();
        self.runs.retain(|r| r.run_id != run_id);
        self.runs.len() != before
    }

    pub fn clear(&mut self) {
        self.runs.clear();
    }
}

/// E03: explicit user preferences — a small bounded key/value store the
/// user sets and can see in full (nothing implicit, nothing hidden).
#[derive(Debug, Default)]
pub struct UserPreferences {
    prefs: Vec<(String, String)>,
}

impl UserPreferences {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set (or replace) one preference; silently drops the request when the
    /// store is full and the key is new (bounded, never grows unbounded).
    pub fn set(&mut self, key: &str, value: &str) {
        let key = bounded(key);
        let value = bounded(value);
        if let Some(slot) = self.prefs.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
            return;
        }
        if self.prefs.len() >= MAX_PREFS {
            return;
        }
        self.prefs.push((key, value));
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.prefs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.prefs.len();
        let key = bounded(key);
        self.prefs.retain(|(k, _)| *k != key);
        self.prefs.len() != before
    }

    /// E04 (user-visible): the full store.
    pub fn all(&self) -> &[(String, String)] {
        &self.prefs
    }

    pub fn clear(&mut self) {
        self.prefs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// E01: bounded per session (FIFO) and in total (oldest session evicted).
    #[test]
    fn session_memory_is_bounded() {
        let mut m = SessionMemory::new();
        for i in 0..MAX_RUNS_PER_SESSION + 5 {
            m.push("s1", "goal", &format!("goal {i}"), i as i64);
        }
        assert_eq!(m.entries("s1").len(), MAX_RUNS_PER_SESSION);
        assert_eq!(m.entries("s1")[0].text, "goal 5", "oldest evicted");
        for i in 0..MAX_SESSIONS + 3 {
            m.push(&format!("s{i}"), "goal", "x", 0);
        }
        assert_eq!(m.session_ids().len(), MAX_SESSIONS);
        assert!(!m.session_ids().contains(&"s1".to_string()), "oldest session forgotten");
    }

    /// E01: entries are control-char sanitized and char-bounded.
    #[test]
    fn entries_sanitized() {
        let mut m = SessionMemory::new();
        m.push("s", "goal", &format!("a\x07b{}", "x".repeat(MAX_ENTRY_CHARS + 50)), 0);
        let e = &m.entries("s")[0];
        assert!(!e.text.contains('\u{7}'), "control characters stripped");
        assert!(e.text.chars().count() <= MAX_ENTRY_CHARS);
        assert!(e.text.starts_with("ab"));
    }

    /// E02: run records bounded FIFO; deletable by id.
    #[test]
    fn run_memory_bounded_and_deletable() {
        let mut m = RunMemory::new();
        for i in 0..MAX_RUNS_PER_SESSION + 2 {
            m.record(RunRecord {
                run_id: format!("r{i}"),
                goal: "g".into(),
                status: "completed".into(),
                turns: 1,
                at_ms: i as i64,
            });
        }
        assert_eq!(m.runs().len(), MAX_RUNS_PER_SESSION);
        assert_eq!(m.runs()[0].run_id, "r2");
        assert!(m.delete_run("r2"));
        assert_eq!(m.runs()[0].run_id, "r3");
        m.clear();
        assert!(m.runs().is_empty());
    }

    /// E03/E04: preferences are explicit, visible, bounded and removable.
    #[test]
    fn preferences_bounded_and_visible() {
        let mut p = UserPreferences::new();
        p.set("theme", "dark");
        p.set("theme", "light");
        assert_eq!(p.get("theme"), Some("light"));
        for i in 0..MAX_PREFS + 5 {
            p.set(&format!("k{i}"), "v");
        }
        assert_eq!(p.all().len(), MAX_PREFS, "new keys beyond cap are refused");
        assert!(p.remove("theme"));
        assert_eq!(p.get("theme"), None);
        assert_eq!(p.all().len(), MAX_PREFS - 1);
    }
}
