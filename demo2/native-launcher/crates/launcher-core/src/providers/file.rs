//! File Provider: answers filename queries from the SQLite index.

use launcher_domain::{Command, QueryContext};
use launcher_indexer::Indexer;

use crate::{file_command, Provider};

/// Per-query result bound from the index.
/// Candidate-set size handed to the global ranker (review 64 §9): must be
/// comfortably larger than the displayed Top-K so global ranking (history
/// boost, type prior) can promote a candidate that the provider-local
/// ordering placed late.
pub const MAX_FILE_HITS: usize = 100;

pub struct FileProvider {
    indexer: Indexer,
}

impl FileProvider {
    pub fn new(indexer: Indexer) -> Self {
        Self { indexer }
    }

    pub fn indexer(&self) -> &Indexer {
        &self.indexer
    }

    pub fn indexer_mut(&mut self) -> &mut Indexer {
        &mut self.indexer
    }
}

impl Provider for FileProvider {
    fn id(&self) -> &str {
        "files"
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            return Vec::new();
        }
        self.indexer
            .search(&q.normalized, MAX_FILE_HITS)
            .unwrap_or_default()
            .iter()
            .map(file_command)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn queries_index() {
        let tmp = std::env::temp_dir().join(format!("fileprov-{}", std::process::id()));
        let root = tmp.join("root");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("invoice 2026.pdf"), b"x").unwrap();
        std::fs::write(root.join("notes.txt"), b"x").unwrap();

        let mut idx = Indexer::in_memory().unwrap();
        idx.rebuild(&[root]).unwrap();
        let mut p = FileProvider::new(idx);

        let q = QueryContext::parse("invoice");
        let hits = p.query(&q);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].provider_id, "files");
        assert_eq!(hits[0].target.as_deref(), Some(hits[0].id.as_str()));
        assert!(!hits[0].actions.is_empty());

        assert!(p.query(&QueryContext::parse("")).is_empty());
        assert_eq!(p.id(), "files");

        let _ = Path::new(&tmp);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
