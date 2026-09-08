//! Search Intelligence v0.1 (P2.5 Batch 1: P25-001 Search Contract v2 +
//! P25-A01 Query Normalizer + P25-A02 Intent Detector + P25-A03 Filters).
//!
//! Spec: `demo2/files2/101-p2.5-0.1.md`. Everything in this module is PURE:
//! no LLM, no network, no filesystem I/O, no provider execution (AC-A02-4 /
//! AC-A01-3). Intents and filters are RETRIEVAL SIGNALS with **no execution
//! authority** (AC-001-3) — the Resolver→Engine chain is untouched.
//!
//! Contract invariants (P25-001):
//! - all v2 DTOs serialize/deserialize stably (AC-001-1);
//! - result states frozen: Complete/Partial/TimedOut/Cancelled/Failed (AC-001-2);
//! - legacy search path keeps working — this layer is additive (AC-001-4).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// P25-001 — Search Contract v2 DTOs
// ---------------------------------------------------------------------------

/// Frozen result states (AC-001-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchResultState {
    Complete,
    Partial,
    TimedOut,
    Cancelled,
    Failed,
}

/// The 8 intent kinds (P25-A02, AC-A02-1). `evidence` records WHY — intent
/// never carries execution authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchIntent {
    Application,
    File,
    Folder,
    Command,
    Plugin,
    Workflow,
    Mixed,
    Unknown,
}

/// Provider-neutral retrieval constraint (P25-A03). A filter constrains
/// WHICH providers see the query; it can never grant capability/authority
/// (AC-A03-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFilter {
    pub kind: FilterKind,
    /// The constrained query text (everything after the `kind:` prefix).
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterKind {
    App,
    File,
    Folder,
    Cmd,
    Plugin,
    Wf,
}

impl FilterKind {
    pub fn parse_prefix(prefix: &str) -> Option<FilterKind> {
        match prefix {
            "app" => Some(FilterKind::App),
            "file" => Some(FilterKind::File),
            "folder" => Some(FilterKind::Folder),
            "cmd" => Some(FilterKind::Cmd),
            "plugin" => Some(FilterKind::Plugin),
            "wf" => Some(FilterKind::Wf),
            _ => None,
        }
    }

    fn to_intent(self) -> SearchIntent {
        match self {
            FilterKind::App => SearchIntent::Application,
            FilterKind::File => SearchIntent::File,
            FilterKind::Folder => SearchIntent::Folder,
            FilterKind::Cmd => SearchIntent::Command,
            FilterKind::Plugin => SearchIntent::Plugin,
            FilterKind::Wf => SearchIntent::Workflow,
        }
    }
}

/// Which providers a request may be routed to (routing planner deepens this
/// in P25-B06; v0.1 keeps it a strategy hint only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStrategy {
    /// Legacy behavior: fan out to every provider (deterministic order).
    All,
    /// Route only to providers whose class matches the intent/filter.
    Routed(Vec<FilterKind>),
}

/// Provider capability declaration (P25-001 step 8) — data, not authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    Discovery,
    Execute,
    Pinyin,
    Fts,
}

/// The v2 search request: everything downstream stages derive from this one
/// frozen structure. Bounded by [`MAX_QUERY_BYTES`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRequestV2 {
    pub raw: String,
    pub normalized: String,
    pub tokens: Vec<String>,
    pub filters: Vec<SearchFilter>,
    pub intent: SearchIntent,
    pub strategy: SearchStrategy,
    /// Soft per-search budget in ms (v0.1: informational; enforced by
    /// P25-B03 timeout isolation later).
    pub budget_ms: u64,
}

/// P25-A01 step 7: hard input bound.
pub const MAX_QUERY_BYTES: usize = 512;

// ---------------------------------------------------------------------------
// P25-A01 — Query Normalizer v2
// ---------------------------------------------------------------------------

/// Normalized query view: raw preserved verbatim, normalized form is
/// lowercased + whitespace-collapsed + bounded, tokens split on whitespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedQuery {
    pub raw: String,
    pub normalized: String,
    pub tokens: Vec<String>,
    /// `*` or `?` present (wildcard semantics land with FTS5, P25-C).
    pub has_wildcard: bool,
    /// Any path separator present (`/` or `\`).
    pub has_path_separator: bool,
}

/// Pure, deterministic (AC-A01-1), Unicode-safe normalizer (AC-A01-2/3/4).
pub fn normalize_query_v2(raw: &str) -> NormalizedQuery {
    let raw = &raw[..raw.floor_char_boundary(MAX_QUERY_BYTES.min(raw.len()))];
    // lowercase + collapse whitespace runs to single spaces (Unicode-aware
    // via char::to_lowercase; CJK has no case, preserved as-is)
    let mut normalized = String::with_capacity(raw.len());
    let mut last_was_space = false;
    for ch in raw.chars() {
        if ch.is_whitespace() {
            // collapse whitespace RUNS to one separator (never leading)
            if !normalized.is_empty() && !last_was_space {
                normalized.push(' ');
            }
            last_was_space = true;
        } else {
            last_was_space = false;
            for lc in ch.to_lowercase() {
                normalized.push(lc);
            }
        }
    }
    while normalized.ends_with(' ') {
        normalized.pop();
    }
    let tokens: Vec<String> = normalized
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();
    NormalizedQuery {
        raw: raw.to_string(),
        has_wildcard: raw.contains('*') || raw.contains('?'),
        has_path_separator: raw.contains('/') || raw.contains('\\'),
        normalized,
        tokens,
    }
}

// ---------------------------------------------------------------------------
// P25-A03 — Explicit filter syntax
// ---------------------------------------------------------------------------

/// Parse the explicit filter grammar: the FIRST token may be a `kind:` prefix
/// (`app:` / `file:` / `folder:` / `cmd:` / `plugin:` / `wf:`); the rest of
/// the query becomes the constrained value. Repeated/combined prefixes: the
/// FIRST recognized prefix wins and later `kind:` tokens stay part of the
/// value (deterministic, AC-A03-1). UNKNOWN prefixes are NOT filters — the
/// whole query stays a literal (safe fallback, AC-A03-2).
pub fn parse_filters(nq: &NormalizedQuery) -> Vec<SearchFilter> {
    let first = match nq.tokens.first() {
        Some(t) => t,
        None => return Vec::new(),
    };
    let Some((prefix, _rest)) = first.split_once(':') else {
        return Vec::new();
    };
    let Some(kind) = FilterKind::parse_prefix(prefix) else {
        return Vec::new(); // unknown prefix → literal fallback
    };
    // value = everything after "<prefix>:" in the NORMALIZED string
    let marker = format!("{prefix}:");
    let value = nq
        .normalized
        .strip_prefix(&marker)
        .unwrap_or("")
        .trim()
        .to_string();
    vec![SearchFilter { kind, value }]
}

// ---------------------------------------------------------------------------
// P25-A02 — Intent Detector
// ---------------------------------------------------------------------------

/// Strong lexical patterns (deterministic; explicit syntax wins first).
const COMMAND_VERBS: [&str; 10] = [
    "open", "run", "copy", "kill", "restart", "shutdown", "sleep", "lock", "eject", "toggle",
];

const DOCUMENT_EXTENSIONS: [&str; 8] =
    ["txt", "md", "pdf", "doc", "docx", "xls", "xlsx", "ppt"];

fn extension_of(token: &str) -> Option<&str> {
    let name = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let (stem_ok, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => (true, ext),
        _ => return None,
    };
    stem_ok.then_some(ext)
}

/// Deterministic intent detection (AC-A02-1..4): explicit filter syntax wins,
/// then structural patterns (path/extension), then command grammar, then a
/// conservative default. Conflicting signals degrade to Mixed; nothing
/// matches → Unknown.
pub fn detect_intent(nq: &NormalizedQuery) -> SearchIntent {
    // 1. explicit syntax has absolute priority (AC-A02-2)
    if let Some(f) = parse_filters(nq).first() {
        return f.kind.to_intent();
    }
    if nq.tokens.is_empty() {
        return SearchIntent::Unknown;
    }
    let first = &nq.tokens[0];
    let mut signals: Vec<SearchIntent> = Vec::new();

    // 2. structural: path separators / trailing separator → Folder
    if nq.has_path_separator {
        let ends_with_sep =
            nq.normalized.ends_with('/') || nq.normalized.ends_with('\\');
        signals.push(if ends_with_sep {
            SearchIntent::Folder
        } else {
            SearchIntent::File
        });
    }

    // 3. extension patterns
    if let Some(ext) = nq.tokens.last().and_then(|t| extension_of(t)) {
        if ext == "exe" || ext == "lnk" {
            signals.push(SearchIntent::Application);
        } else if DOCUMENT_EXTENSIONS.contains(&ext) {
            signals.push(SearchIntent::File);
        }
    }

    // 4. command grammar: verb-led multi-token queries
    if nq.tokens.len() >= 2 && COMMAND_VERBS.contains(&first.as_str()) {
        signals.push(SearchIntent::Command);
    }

    match signals.as_slice() {
        [] => {
            // conservative default: a bare word is app-biased
            SearchIntent::Application
        }
        [only] => only.clone(),
        many => {
            if many.iter().all(|s| *s == many[0]) {
                many[0].clone()
            } else {
                SearchIntent::Mixed // safe degradation on conflict (AC-A02-3)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Request assembly (compatibility bridge: legacy string queries stay valid)
// ---------------------------------------------------------------------------

/// Assemble a v2 request from a legacy raw query string (AC-001-4: the old
/// `&str` entry points remain the source of truth; v2 is derived).
pub fn search_request_v2(raw: &str, budget_ms: u64) -> SearchRequestV2 {
    let nq = normalize_query_v2(raw);
    let filters = parse_filters(&nq);
    let intent = detect_intent(&nq);
    let strategy = match filters.first() {
        Some(f) => SearchStrategy::Routed(vec![f.kind]),
        None => SearchStrategy::All,
    };
    SearchRequestV2 {
        raw: nq.raw,
        normalized: nq.normalized,
        tokens: nq.tokens,
        filters,
        intent,
        strategy,
        budget_ms: budget_ms.min(10_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- A01 normalizer ---------------------------------------------------

    #[test]
    fn a01_empty_and_spaces() {
        assert_eq!(normalize_query_v2("").normalized, "");
        assert!(normalize_query_v2("").tokens.is_empty());
        assert_eq!(normalize_query_v2("   ").normalized, "");
        assert_eq!(normalize_query_v2("  a   b  ").normalized, "a b");
        assert_eq!(normalize_query_v2("  a   b  ").tokens, vec!["a", "b"]);
    }

    #[test]
    fn a01_unicode_chinese_english() {
        let q = normalize_query_v2("Chrome 浏览器");
        assert_eq!(q.normalized, "chrome 浏览器");
        assert_eq!(q.tokens, vec!["chrome", "浏览器"]);
        // full-width space is whitespace
        assert_eq!(normalize_query_v2("a\u{3000}b").normalized, "a b");
    }

    #[test]
    fn a01_path_and_wildcard() {
        let q = normalize_query_v2(r"C:\Users\Spence\Report.PDF");
        assert_eq!(q.normalized, r"c:\users\spence\report.pdf");
        assert!(q.has_path_separator);
        assert!(!q.has_wildcard);
        assert!(normalize_query_v2("rep*.pdf").has_wildcard);
        assert!(normalize_query_v2("rep?.pdf").has_wildcard);
    }

    /// AC-A01-4: input is hard-bounded at MAX_QUERY_BYTES.
    #[test]
    fn a01_long_input_bounded() {
        let long = "a".repeat(10_000);
        let q = normalize_query_v2(&long);
        assert!(q.normalized.len() <= MAX_QUERY_BYTES);
    }

    /// AC-A01-1: 1000x deterministic replay.
    #[test]
    fn a01_deterministic_replay() {
        let raw = "Chrome 浏览器 C:\\Users\\r*.pdf";
        let first = normalize_query_v2(raw);
        for _ in 0..1000 {
            assert_eq!(normalize_query_v2(raw), first);
        }
    }

    // ---- A03 filters --------------------------------------------------------

    #[test]
    fn a03_all_prefixes_parse() {
        for (prefix, kind) in [
            ("app", FilterKind::App),
            ("file", FilterKind::File),
            ("folder", FilterKind::Folder),
            ("cmd", FilterKind::Cmd),
            ("plugin", FilterKind::Plugin),
            ("wf", FilterKind::Wf),
        ] {
            let q = normalize_query_v2(&format!("{prefix}:chrome"));
            let f = parse_filters(&q);
            assert_eq!(f.len(), 1, "{prefix}");
            assert_eq!(f[0].kind, kind);
            assert_eq!(f[0].value, "chrome");
        }
    }

    /// AC-A03-2: unknown prefixes are literal text, not filters.
    #[test]
    fn a03_unknown_prefix_falls_back() {
        let q = normalize_query_v2("foo:bar");
        assert!(parse_filters(&q).is_empty());
        assert_eq!(q.normalized, "foo:bar");
        // a colon mid-query without a recognized leading prefix is literal
        let q = normalize_query_v2("chrome:profile");
        assert!(parse_filters(&q).is_empty());
    }

    /// AC-A03-1: filter value keeps the rest of the query (repeated tokens
    /// and unicode included); empty value allowed (kind-only filter).
    #[test]
    fn a03_value_and_empty_value() {
        let q = normalize_query_v2("file: quarterly 报告.pdf");
        let f = parse_filters(&q);
        assert_eq!(f[0].kind, FilterKind::File);
        assert_eq!(f[0].value, "quarterly 报告.pdf");
        let q = normalize_query_v2("app:");
        assert_eq!(parse_filters(&q)[0].value, "");
    }

    // ---- A02 intent ---------------------------------------------------------

    #[test]
    fn a02_explicit_syntax_wins() {
        // even though "run report" looks like a command, the filter decides
        let req = search_request_v2("file: run report", 100);
        assert_eq!(req.intent, SearchIntent::File);
        assert_eq!(req.strategy, SearchStrategy::Routed(vec![FilterKind::File]));
    }

    #[test]
    fn a02_eight_kinds_covered() {
        let cases = [
            ("chrome", SearchIntent::Application),
            ("report.pdf", SearchIntent::File),
            ("c:\\users\\spence\\", SearchIntent::Folder),
            ("restart now", SearchIntent::Command),
            ("plugin:weather", SearchIntent::Plugin),
            ("wf:daily-backup", SearchIntent::Workflow),
        ];
        for (raw, want) in cases {
            assert_eq!(search_request_v2(raw, 100).intent, want, "{raw}");
        }
    }

    #[test]
    fn a02_conflict_degrades_to_mixed_and_unknown() {
        // path separator + document extension + no trailing sep = File/Files
        // agree; conflict case: verb-led AND extension
        let q = normalize_query_v2("open notes.md");
        assert_eq!(detect_intent(&q), SearchIntent::Mixed);
        // empty → Unknown
        assert_eq!(detect_intent(&normalize_query_v2("")), SearchIntent::Unknown);
    }

    #[test]
    fn a02_deterministic() {
        let raw = "open c:\\reports\\q4.pdf";
        let first = search_request_v2(raw, 50);
        for _ in 0..100 {
            assert_eq!(search_request_v2(raw, 50), first);
        }
    }

    // ---- P25-001 contract ---------------------------------------------------

    /// AC-001-1: v2 DTOs roundtrip stably.
    #[test]
    fn contract_request_roundtrips() {
        let req = search_request_v2("file: quarterly 报告.pdf", 250);
        let json = serde_json::to_string(&req).unwrap();
        let back: SearchRequestV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, req);
    }

    /// AC-001-2: result states frozen (serde names pinned).
    #[test]
    fn contract_result_states_frozen() {
        let states = [
            (SearchResultState::Complete, "\"complete\""),
            (SearchResultState::Partial, "\"partial\""),
            (SearchResultState::TimedOut, "\"timed_out\""),
            (SearchResultState::Cancelled, "\"cancelled\""),
            (SearchResultState::Failed, "\"failed\""),
        ];
        for (v, name) in states {
            assert_eq!(serde_json::to_string(&v).unwrap(), name);
            let back: SearchResultState = serde_json::from_str(name).unwrap();
            assert_eq!(back, v);
        }
    }

    /// AC-001-3 (structural pin): intent/filter/strategy DTOs carry no
    /// capability or authority fields — asserted via the frozen serde shape
    /// of a full request.
    #[test]
    fn contract_request_shape_is_authority_free() {
        let req = search_request_v2("app:chrome", 100);
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        let keys: Vec<String> = v
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.to_string())
            .collect();
        assert_eq!(
            keys,
            vec!["budget_ms", "filters", "intent", "normalized", "raw", "strategy", "tokens"]
        );
        // capability/authority must NEVER appear in the request contract
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("capabilit"));
        assert!(!json.contains("authority"));
    }

    /// AC-001-4: legacy path unaffected — normalizer accepts arbitrary legacy
    /// queries and the v2 layer is derivable, not required.
    #[test]
    fn contract_legacy_queries_still_work() {
        for raw in ["", "chrome", r"c:\x\y.txt", "spawn **weird** stuff"] {
            let req = search_request_v2(raw, 100);
            assert_eq!(req.raw, raw);
        }
    }
}
