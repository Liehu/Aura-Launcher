//! P2.1-B support components (review 76 §11-21/§35): bounded event queue,
//! identity-based coalescer, dirty-root set. All are pure/bounded and
//! unit-testable without filesystem or SQLite access.

use std::collections::HashMap;

use launcher_domain::{FileChange, FileChangeKind};

/// Path identity for one file-change event; `None` when the event carries no
/// usable identity (empty path).
fn change_identities(c: &FileChange) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(old) = &c.old_path {
        let id = launcher_domain::normalize_path_identity(old);
        if !id.is_empty() {
            ids.push(id);
        }
    }
    let id = launcher_domain::normalize_path_identity(&c.path);
    if !id.is_empty() {
        ids.push(id);
    }
    ids
}

/// INV-INDEX-001: hard capacity. Pushing beyond capacity returns `false` —
/// the caller records a dirty root and drops redundant events instead of
/// growing memory (review 76 §35/§37).
pub struct BoundedQueue {
    capacity: usize,
    events: std::collections::VecDeque<FileChange>,
    pub dropped: u64,
}

impl BoundedQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            events: std::collections::VecDeque::new(),
            dropped: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns `false` when the queue was full (event NOT stored).
    pub fn push(&mut self, e: FileChange) -> bool {
        if self.events.len() >= self.capacity {
            self.dropped += 1;
            return false;
        }
        self.events.push_back(e);
        true
    }

    pub fn drain(&mut self) -> Vec<FileChange> {
        self.events.drain(..).collect()
    }
}

/// Coalescer (review 76 §11-13): reduces event bursts to the set of distinct
/// path identities needing verification. Because every identity is re-statted
/// by the Coordinator anyway ("event is a hint"), the correct minimal output
/// is the unique identity set — Created+Modified+Deleted collapses to one
/// entry, renames register BOTH old and new identities, and semantically
/// identical paths (`C:\Foo`, `c:/foo`) share one entry via
/// `normalize_path_identity`.
#[derive(Default)]
pub struct Coalescer {
    order: Vec<String>,
    seen: HashMap<String, FileChangeKind>,
    count_in: u64,
}

impl Coalescer {
    pub fn push(&mut self, c: FileChange) {
        self.count_in += 1;
        for id in change_identities(&c) {
            if !self.seen.contains_key(&id) {
                self.order.push(id.clone());
            }
            // Deleted wins as the hint for the identity (verification will
            // re-stat anyway); kind recorded for diagnostics only.
            let kind = match c.kind {
                FileChangeKind::Deleted | FileChangeKind::Renamed => Some(c.kind),
                _ => self.seen.get(&id).copied().or(Some(c.kind)),
            };
            if let Some(k) = kind {
                self.seen.insert(id, k);
            }
        }
    }

    /// Distinct identities pending verification (order of first appearance).
    pub fn pending(&self) -> &[String] {
        &self.order
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn count_in(&self) -> u64 {
        self.count_in
    }

    pub fn drain(&mut self) -> Vec<String> {
        self.count_in = 0;
        self.seen.clear();
        std::mem::take(&mut self.order)
    }
}

/// Dirty-root set with path-containment coalescing (review 76 §14/15/20):
/// `C:\A`, `C:\A\B`, `C:\A\B\C` collapse to `C:\A`; siblings stay separate.
#[derive(Default)]
pub struct DirtyRootSet {
    roots: Vec<String>,
}

impl DirtyRootSet {
    pub fn insert(&mut self, root: &str) {
        let id = launcher_domain::normalize_path_identity(root);
        if id.is_empty() {
            return;
        }
        // covered by an existing root (or equal)?
        if self.contains_or_parent(&id) {
            return;
        }
        // new root covers existing ones -> drop them
        let prefix = format!("{id}\\");
        self.roots.retain(|r| !r.starts_with(&prefix));
        self.roots.push(id);
    }

    /// True when `path` is inside a dirty root (or IS one).
    pub fn contains_or_parent(&self, path: &str) -> bool {
        let id = launcher_domain::normalize_path_identity(path);
        self.roots
            .iter()
            .any(|r| id == *r || id.starts_with(&format!("{r}\\")))
    }

    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn drain(&mut self) -> Vec<String> {
        std::mem::take(&mut self.roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc(kind: FileChangeKind, path: &str) -> FileChange {
        FileChange {
            kind,
            path: path.into(),
            old_path: None,
        }
    }

    /// INV-INDEX-001: hard capacity, dropped counter, drain.
    #[test]
    fn bounded_queue_enforces_capacity() {
        let mut q = BoundedQueue::new(3);
        for i in 0..5 {
            let ok = q.push(fc(FileChangeKind::Created, &format!("C:\\f{i}.txt")));
            if i >= 3 {
                assert!(!ok, "pushes beyond capacity must be rejected");
            } else {
                assert!(ok);
            }
        }
        assert_eq!(q.len(), 3);
        assert_eq!(q.dropped, 2);
        assert_eq!(q.drain().len(), 3);
        assert!(q.is_empty());
    }

    /// review 76 §11-12: a burst on one path collapses to ONE identity;
    /// rename registers both old and new; case/separator variants merge.
    #[test]
    fn coalescer_collapses_bursts_and_renames() {
        let mut c = Coalescer::default();
        c.push(fc(FileChangeKind::Created, r"C:\Work\a.txt"));
        c.push(fc(FileChangeKind::Modified, r"C:\Work\a.txt"));
        c.push(fc(FileChangeKind::Modified, r"c:\work\a.txt"));
        assert_eq!(c.len(), 1, "burst on one identity collapses");

        c.push(FileChange {
            kind: FileChangeKind::Renamed,
            path: r"C:\Work\b.txt".into(),
            old_path: Some(r"C:\Work\a.txt".into()),
        });
        assert_eq!(c.len(), 2, "rename adds the new identity");
        assert!(c.pending().contains(&launcher_domain::normalize_path_identity(r"C:\Work\a.txt")));
        assert!(c.pending().contains(&launcher_domain::normalize_path_identity(r"C:\Work\b.txt")));

        c.push(fc(FileChangeKind::Modified, r"C:\Work\c.txt"));
        let drained = c.drain();
        assert_eq!(drained.len(), 3);
        assert_eq!(c.len(), 0);
        assert_eq!(c.count_in(), 0);
    }

    /// review 76 §15/20: containment coalescing — ancestor wins, siblings
    /// stay separate.
    #[test]
    fn dirty_root_set_containment_merge() {
        let mut d = DirtyRootSet::default();
        d.insert(r"C:\data\projects");
        d.insert(r"C:\data\projects\foo");
        d.insert(r"C:\data\projects\foo\bar");
        assert_eq!(d.len(), 1, "nested dirty roots collapse to the ancestor");

        d.insert(r"C:\data\other");
        assert_eq!(d.len(), 2, "siblings stay separate");

        assert!(d.contains_or_parent(r"C:\data\projects\foo\new.txt"));
        assert!(!d.contains_or_parent(r"C:\somewhere\else"));
        let drained = d.drain();
        assert_eq!(drained.len(), 2);
        assert!(d.is_empty());
    }
}
