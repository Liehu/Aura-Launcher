//! Deterministic scoring, deduplication and ranking (design spec 7.3).
//!
//! pure functions only, fully unit-testable, no IO.

pub mod cache;

use std::path::Path;

use launcher_domain::{Category, Command, QueryContext};

/// Result-type prior (07-mvp2-0.3 §10): when text similarity is close, an
/// application should outrank a raw file — `chrome.exe` beats
/// `chrome-icon.svg` even at equal string match. Applied only when the
/// command matched at all (score > 0).
fn type_prior(category: Category) -> f32 {
    match category {
        Category::Application => 25.0,
        Category::Folder => 20.0,
        Category::Command => 15.0,
        Category::Plugin => 10.0,
        Category::File => 0.0,
    }
}


/// P2.1-G (review 70 §49/52): ranking contributions as DATA, not scattered
/// magic numbers — benchmarkable and single-sourced. Defaults are the values
/// that have been frozen in tests since MVP2; changing a weight must keep
/// `exact > prefix > contains` and "boost never creates candidates" true.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankingWeights {
    pub keyword_exact: f32,
    pub keyword_prefix: f32,
    pub keyword_contains: f32,
    pub fuzzy_subsequence: f32,
    pub multi_token: f32,
    /// Per-use history bonus, capped at `history_freq_cap` uses.
    pub history_freq_per_use: f32,
    pub history_freq_cap: u32,
    /// Recency bonus: used within 24h / within 7 days.
    pub history_recency_day: f32,
    pub history_recency_week: f32,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            keyword_exact: 50.0,
            keyword_prefix: 25.0,
            keyword_contains: 15.0,
            fuzzy_subsequence: 20.0,
            multi_token: 30.0,
            history_freq_per_use: 3.0,
            history_freq_cap: 10,
            history_recency_day: 12.0,
            history_recency_week: 6.0,
        }
    }
}

/// Score a single command against a parsed query.
///
/// exact > prefix > contains > fuzzy, scored over candidate views (full
/// title, file stem, each title word) so "Google Chrome" matches "ch" via
/// the word "chrome" without extension noise from "chrome-shortcut.url",
/// plus per-keyword bonuses and the result-type prior.
pub fn score(command: &Command, query: &QueryContext) -> f32 {
    if query.normalized.is_empty() {
        return 0.0;
    }
    // plugin hint (bounded [0,1], weighted) — lets non-lexical results like
    // calculators surface without letting a plugin own the ranking:
    // max hint contribution 45 < exact title match 100 (ADR-0011).
    let mut s = command.score.clamp(0.0, 1.0) * 45.0;
    let title = command.title.to_lowercase();
    let q = &query.normalized;

    let mut title_score = best_title_score(&title, q);
    if command.category == Category::File {
        if let Some(stem) = Path::new(&title).file_stem().and_then(|s| s.to_str()) {
            title_score = title_score.max(best_title_score(stem, q));
        }
    }
    for w in title.split(|c: char| c.is_whitespace() || matches!(c, '-' | '_' | '.')) {
        title_score = title_score.max(best_title_score(w, q));
    }
    s += title_score;

    // per-keyword bonus (weights: P2.1-G)
    let w = RankingWeights::default();
    for k in &command.keywords {
        let k = k.to_lowercase();
        s += if *k == *q {
            w.keyword_exact
        } else if k.starts_with(q) {
            w.keyword_prefix
        } else if k.contains(q) {
            w.keyword_contains
        } else {
            0.0
        };
    }

    // token-level (subsequence fuzzy) match
    if s == 0.0 && is_subsequence(&title, q) {
        s += w.fuzzy_subsequence;
    }
    // every token must appear somewhere for multi-token queries
    if query.tokens.len() > 1 {
        let keywords = command
            .keywords
            .iter()
            .map(|k| k.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        let haystack = format!("{title} {keywords}");
        let all = query.tokens.iter().all(|t| haystack.contains(t));
        if all {
            s += w.multi_token;
        }
    }
    if s > 0.0 {
        s += type_prior(command.category);
    }
    s
}

/// exact > prefix > contains score for one candidate string.
fn best_title_score(candidate: &str, query: &str) -> f32 {
    if candidate == query {
        100.0
    } else if candidate.starts_with(query) {
        60.0
    } else if candidate.contains(query) {
        40.0
    } else {
        0.0
    }
}

fn is_subsequence(haystack: &str, needle: &str) -> bool {
    let mut it = haystack.chars();
    for n in needle.chars() {
        if !it.any(|h| h == n) {
            return false;
        }
    }
    true
}

/// P2.1-C semantic identity key (review 77 §4-5/§28: IdentityKey v1 — a
/// stable String, not a typed enum yet): File/Folder/Application candidates
/// are identified by the NORMALIZED TARGET PATH, so the same object found by
/// different providers collapses to one result; everything else (commands,
/// plugins, MCP tools) is identified by (provider_id, id). Identity is
/// derived from host-resolved targets only — never from plugin metadata.
pub fn search_identity_key(c: &Command) -> String {
    match (c.category, c.target.as_deref()) {
        (Category::Application | Category::File | Category::Folder, Some(t)) => {
            // one shared path namespace: an application and a File hit on
            // the same exe ARE the same object to the user
            format!("path:{}", launcher_domain::normalize_path_identity(t))
        }
        _ => format!("cmd:{}:{}", c.provider_id, c.id),
    }
}

/// Deduplicate by (provider_id, id), keeping the highest-scored entry,
/// sort descending (stable, deterministic) and truncate.
pub fn rank(commands: Vec<Command>, query: &QueryContext, limit: usize) -> Vec<Command> {
    rank_with_boost(commands, query, limit, |_| 0.0)
}

/// [`rank`] with an additional usage/frequency boost (P2-D): `boost` is
/// added ONLY to commands that already matched lexically (score > 0), so a
/// frequently-used command moves up within its match class but never
/// surfaces unrelated noise on a fresh query. Deterministic: boost is a
/// pure function of the command.
pub fn rank_with_boost(
    mut commands: Vec<Command>,
    query: &QueryContext,
    limit: usize,
    boost: impl Fn(&Command) -> f32,
) -> Vec<Command> {
    for c in commands.iter_mut() {
        let base = score(c, query);
        c.score = if base > 0.0 { base + boost(c) } else { 0.0 };
    }
    commands.retain(|c| c.score > 0.0);
    commands.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.provider_id.cmp(&b.provider_id))
            .then_with(|| a.id.cmp(&b.id))
    });
    // P2-FIX-02: ACTUALLY deduplicate by (provider_id, id) — the doc
    // comment always promised this but the old code never did it. After
    // the deterministic sort the first entry of each key is the
    // highest-scored one.
    // pass 1: same provider returning the same item twice
    let mut seen = std::collections::HashSet::new();
    commands.retain(|c| seen.insert((c.provider_id.clone(), c.id.clone())));
    // pass 2: P2.1-C semantic identity dedup — the same FILE/APP discovered
    // by different providers (Start Menu vs Recent vs File index) collapses
    // to the highest-scored entry, MERGING missing presentation metadata
    // (subtitle) from the dropped rows (review 82 §3/§18: identity collapse
    // must not silently discard source metadata).
    // Candidate Merge (P2.1-C Gate C7 / review 82 §6): same semantic
    // identity collapses into the highest-scored CANONICAL candidate, but
    // presentation metadata (subtitle) and ACTION DESCRIPTORS from dropped
    // rows are merged in — identity collapse must not discard source
    // metadata or alternative actions.
    let mut identities: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut merge_pairs: Vec<(usize, usize)> = Vec::new(); // (kept_idx, dropped_idx)
    let mut keep_set: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (i, c) in commands.iter().enumerate() {
        let id = search_identity_key(c);
        match identities.get(&id) {
            Some(&kept_idx) => merge_pairs.push((kept_idx, i)),
            None => {
                identities.insert(id, i);
                keep_set.insert(i);
            }
        }
    }
    // merge dropped rows into their kept canonical row (deferred to satisfy
    // the borrow checker)
    for (kept_idx, dropped_idx) in &merge_pairs {
        let (kept, dropped) = if kept_idx < dropped_idx {
            let (l, r) = commands.split_at_mut(*dropped_idx);
            (&mut l[*kept_idx], &mut r[0])
        } else {
            let (l, r) = commands.split_at_mut(*kept_idx);
            (&mut r[0], &mut l[*dropped_idx])
        };
        if kept.subtitle.is_none() && dropped.subtitle.is_some() {
            kept.subtitle = dropped.subtitle.clone();
        }
        for a in &dropped.actions {
            let aid = a.id.as_deref().unwrap_or("");
            if !kept
                .actions
                .iter()
                .any(|k| k.id.as_deref().unwrap_or("") == aid && k.kind == a.kind)
            {
                kept.actions.push(a.clone());
            }
        }
    }
    let mut merge_idx = 0usize;
    commands.retain(|_| {
        let keep_this = keep_set.contains(&merge_idx);
        merge_idx += 1;
        keep_this
    });
    commands.truncate(limit);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin hint (score 1.0) must survive the rank filter even with zero
    /// lexical match — this is what makes calculator-style plugins visible.
    #[test]
    fn hinted_plugin_result_survives_rank_without_lexical_match() {
        let plugin = Command {
            id: "calc.plus:= 400".into(),
            title: "= 400".into(),
            subtitle: None,
            icon: None,
            provider_id: "plugin:calc.plus".into(),
            score: 1.0,
            keywords: vec![],
            category: Category::Plugin,
            actions: vec![],
            target: None,
        };
        let file = Command {
            id: "f".into(),
            // contains-match only (40) < max hint (45+prior): the intended
            // comparison; a title *word* equal to the query would be exact 100
            title: "v20*201 backup.txt".into(),
            subtitle: None,
            icon: None,
            provider_id: "files".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::File,
            actions: vec![],
            target: None,
        };
        let ranked = rank(vec![plugin, file], &QueryContext::parse("20*20"), 10);
        assert_eq!(
            ranked[0].id, "calc.plus:= 400",
            "hinted result must rank first"
        );
        // the lexically-matching file survives too (hint does not crowd out)
        assert!(
            ranked.iter().any(|c| c.id == "f"),
            "lexical match must survive"
        );
    }

    #[test]
    fn hint_is_bounded_and_cannot_hijack_exact_matches() {
        let hinted = Command {
            id: "hinted".into(),
            title: "unrelated title".into(), // zero lexical match
            subtitle: None,
            icon: None,
            provider_id: "plugin:x".into(),
            score: 1.0, // max hint (out-of-range values are clamped upstream)
            keywords: vec![],
            category: Category::Plugin,
            actions: vec![],
            target: None,
        };
        let exact = Command {
            id: "exact".into(),
            title: "y".into(), // exact lexical match, no hint
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![],
            target: None,
        };
        let ranked = rank(vec![hinted, exact], &QueryContext::parse("y"), 10);
        assert_eq!(
            ranked[0].id, "exact",
            "max hint must never beat an exact lexical match"
        );
    }

    use launcher_domain::{Action, ActionKind, Category};

    fn cmd(id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        }
    }

    #[test]
    fn empty_query_scores_zero() {
        let q = QueryContext::parse("");
        assert_eq!(score(&cmd("1", "code"), &q), 0.0);
    }

    #[test]
    fn exact_beats_prefix_beats_contains() {
        let q = QueryContext::parse("code");
        let exact = cmd("e", "code");
        let prefix = cmd("p", "codeblocks");
        let contains = cmd("c", "vscode");
        assert!(score(&exact, &q) > score(&prefix, &q));
        assert!(score(&prefix, &q) > score(&contains, &q));
    }

    #[test]
    fn case_insensitive_and_unicode() {
        let q = QueryContext::parse("记事本");
        assert!(score(&cmd("1", "记事本 Notepad"), &q) > 0.0);
        let q2 = QueryContext::parse("NOTE");
        assert!(score(&cmd("2", "notepad"), &q2) > 0.0);
    }

    #[test]
    fn fuzzy_subsequence() {
        let q = QueryContext::parse("vsc");
        assert!(score(&cmd("1", "visual studio code"), &q) > 0.0);
    }

    #[test]
    fn rank_orders_dedups_limits() {
        let q = QueryContext::parse("te");
        let cmds = vec![
            cmd("a", "test"),
            Command {
                provider_id: "files".into(),
                ..cmd("a", "test")
            },
            cmd("b", "terminal"),
            cmd("c", "unrelated"),
        ];
        let out = rank(cmds, &q, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "test");
        assert!(out.iter().all(|c| c.score > 0.0));
    }

    #[test]
    fn rank_is_deterministic_regardless_of_input_order() {
        let q = QueryContext::parse("te");
        let a = rank(vec![cmd("b", "terminal"), cmd("a", "test")], &q, 10);
        let b = rank(vec![cmd("a", "test"), cmd("b", "terminal")], &q, 10);
        let titles = |v: Vec<Command>| -> Vec<String> { v.into_iter().map(|c| c.title).collect() };
        assert_eq!(titles(a), titles(b));
    }
}

#[cfg(test)]
mod boost_tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category};

    fn cmd(id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
            confirmation_required: false,
            }],
            target: None,
        }
    }

    /// P2-D: usage boost reorders WITHIN a match class but must never lift
    /// a non-matching command above score 0 (no lexical match, no surfacing).
    #[test]
    fn boost_reorders_within_match_class_only() {
        let q = QueryContext::parse("ch");
        let heavy = cmd("chrome", "Google Chrome"); // word-prefix match (60)
        let plain = cmd("checksum", "checksum utility"); // word-prefix too
        let boost = |c: &Command| if c.id == "chrome" { 30.0 } else { 0.0 };
        let out = rank_with_boost(vec![plain.clone(), heavy], &q, 10, boost);
        assert_eq!(out[0].id, "chrome", "boost lifts the used command");

        // unrelated frequently-used command stays invisible on this query
        let unused = cmd("other", "unrelated thing");
        let out = rank_with_boost(vec![unused, plain], &q, 10, boost);
        assert!(!out.iter().any(|c| c.id == "other"));
    }

    #[test]
    fn rank_delegates_with_zero_boost() {
        let q = QueryContext::parse("te");
        let a = rank(vec![cmd("b", "terminal"), cmd("a", "test")], &q, 10);
        let b = rank_with_boost(
            vec![cmd("b", "terminal"), cmd("a", "test")],
            &q,
            10,
            |_| 0.0,
        );
        assert_eq!(a, b);
    }
}


#[cfg(test)]
mod dedup_tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category};

    fn cmd(provider: &str, id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: provider.into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        }
    }

    /// P2-FIX-02: same (provider_id, id) arriving twice (e.g. Start Menu +
    /// registry + portable) must appear ONCE, keeping the highest score.
    #[test]
    fn duplicate_identity_deduplicated_keeping_best_score() {
        let q = QueryContext::parse("chrome");
        let dupes = vec![
            cmd("apps", "chrome", "Chrome"),      // exact title (100)
            cmd("apps", "chrome", "Chromatics"),  // prefix only (60)
            cmd("apps", "chrome", "My Chromatica"), // contains only (40)
        ];
        let out = rank(dupes, &q, 10);
        assert_eq!(out.len(), 1, "duplicate identity must collapse to one row");
        assert_eq!(out[0].title, "Chrome", "highest-scored entry wins");
    }
}

#[cfg(test)]
mod identity_dedup_tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category};

    fn cmd(provider: &str, id: &str, title: &str, category: Category, target: Option<&str>) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: provider.into(),
            score: 0.0,
            keywords: vec![],
            category,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: target.map(str::to_string),
        }
    }

    /// P2.1-C Gate C6: the same application discovered by two providers
    /// (Start Menu shortcut + Recent exe) collapses to ONE result.
    #[test]
    fn same_app_across_providers_dedup() {
        let q = QueryContext::parse("chrome");
        let app = cmd("app-registry", "appreg:lnk", "Google Chrome", Category::Application,
                      Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"));
        let recent = cmd("recent", "recent:chrome", "chrome", Category::File,
                         Some(r"C:\PROGRAM FILES\Google\Chrome\Application\chrome.EXE"));
        let out = rank(vec![app, recent], &q, 10);
        assert_eq!(out.len(), 1, "same exe identity must merge across providers");
        assert_eq!(out[0].provider_id, "app-registry", "highest-scored source wins");
    }

    /// P2.1-C Gate C35: same basename, different directories — two distinct
    /// applications must NOT merge.
    #[test]
    fn same_basename_different_dirs_stay_separate() {
        let q = QueryContext::parse("chrome");
        let a = cmd("apps", "a", "chrome", Category::Application,
                    Some(r"C:\Chrome\chrome.exe"));
        let b = cmd("apps", "b", "chrome", Category::Application,
                    Some(r"D:\Tools\chrome.exe"));
        let out = rank(vec![a, b], &q, 10);
        assert_eq!(out.len(), 2, "full-path identity must not collide on basename");
    }

    /// Files: same path via two providers merges; distinct paths don't.
    #[test]
    fn file_identity_dedup_by_normalized_path() {
        let q = QueryContext::parse("report");
        let f1 = cmd("files", "p1", "report.pdf", Category::File,
                     Some(r"C:\docs\report.pdf"));
        let f2 = cmd("recent", "p2", "report.pdf", Category::File,
                     Some(r"c:/docs/report.pdf"));
        let out = rank(vec![f1, f2], &q, 10);
        assert_eq!(out.len(), 1);
    }

    /// Commands/plugins keep per-provider identity (no cross-provider merge).
    #[test]
    fn commands_not_merged_across_providers() {
        let _q = QueryContext::parse("hash");
        let q2 = QueryContext::parse("sha");
        let p1 = cmd("plugin:hash", "sha256", "sha256", Category::Plugin, None);
        let p2 = cmd("plugin:other", "sha256", "sha256", Category::Plugin, None);
        let out = rank(vec![p1, p2], &q2, 10);
        assert_eq!(out.len(), 2, "command identity is provider-scoped");
    }
}

// ---- P2.2-C: Context-aware ranking signals (pure, deterministic) --------

/// Discrete time bucket (review: P2.2-C §11/§42) — a weak signal, never
/// persisted, bucketed so the context generation stays stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeBucket {
    Night,
    Morning,
    Afternoon,
    Evening,
}

impl TimeBucket {
    pub fn from_hour(h: u32) -> Self {
        match h {
            0..=7 => TimeBucket::Night,
            8..=11 => TimeBucket::Morning,
            12..=17 => TimeBucket::Afternoon,
            _ => TimeBucket::Evening,
        }
    }
}

/// One search request's context, captured ONCE before ranking (§4/§15).
/// Both fields are host-resolved; `None` = unknown (never guessed).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextSnapshot {
    /// Foreground application identity (e.g. executable path or app name)
    /// at invocation time.
    pub foreground_app: Option<String>,
    /// Explorer folder at invocation time (normalized on use).
    pub current_folder: Option<String>,
}

/// Structural relation between a candidate path and the current folder
/// (§21/§24): SameFolder > Descendant > Ancestor > None.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FolderProximity {
    None,
    Ancestor,
    Descendant,
    SameFolder,
}

/// Weights for context score (§26): added to base score, never replacing
/// text relevance (§25). Defaults are small vs lexical scores.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContextRankingWeights {
    pub foreground_match: f32,
    pub same_folder: f32,
    pub descendant_folder: f32,
    pub ancestor_folder: f32,
}

impl Default for ContextRankingWeights {
    fn default() -> Self {
        Self {
            foreground_match: 8.0,
            same_folder: 12.0,
            descendant_folder: 7.0,
            ancestor_folder: 3.0,
        }
    }
}

/// Pure context score for one candidate (§30/§95/§96): O(path), no IO, no
/// candidate creation (§58/§112). Returns 0 when context fields are None.
pub fn context_score(
    command: &Command,
    context: &ContextSnapshot,
    weights: &ContextRankingWeights,
) -> f32 {
    let mut score = 0.0;
    // foreground app match: candidate identity vs foreground identity
    if let (Some(fg), Some(target)) = (&context.foreground_app, &command.target) {
        if launcher_domain::normalize_path_identity(fg)
            == launcher_domain::normalize_path_identity(target)
        {
            score += weights.foreground_match;
        }
    }
    // folder proximity for path-bearing candidates
    if let (Some(folder), Some(target)) = (&context.current_folder, &command.target) {
        let f = launcher_domain::normalize_path_identity(folder);
        let t = launcher_domain::normalize_path_identity(target);
        // candidate PARENT (not the candidate itself) determines proximity
        let parent = match t.rsplit_once('\\') {
            Some((p, _)) => p.to_string(),
            None => String::new(),
        };
        if !f.is_empty() && !t.is_empty() {
            let proximity = if parent == f {
                FolderProximity::SameFolder
            } else if t.starts_with(&format!("{f}\\")) {
                FolderProximity::Descendant
            } else if f.starts_with(&format!("{t}\\")) {
                FolderProximity::Ancestor
            } else {
                FolderProximity::None
            };
            score += match proximity {
                FolderProximity::SameFolder => weights.same_folder,
                FolderProximity::Descendant => weights.descendant_folder,
                FolderProximity::Ancestor => weights.ancestor_folder,
                FolderProximity::None => 0.0,
            };
        }
    }
    score
}

#[cfg(test)]
mod context_tests {
    use super::*;
    use launcher_domain::{Action, ActionKind};

    fn file_cmd(path: &str, title: &str) -> Command {
        Command {
            id: path.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "files".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::File,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some(path.into()),
        }
    }

    fn ctx(folder: Option<&str>, fg: Option<&str>) -> ContextSnapshot {
        ContextSnapshot {
            foreground_app: fg.map(str::to_string),
            current_folder: folder.map(str::to_string),
        }
    }

    /// CTX-001/§60: same-folder candidate outranks an unrelated-folder
    /// candidate at equal text score.
    #[test]
    fn same_folder_beats_unrelated() {
        let c = ctx(Some(r"C:\Projects\Launcher"), None);
        let w = ContextRankingWeights::default();
        let a = file_cmd(r"C:\Projects\Launcher\README.md", "README.md");
        let b = file_cmd(r"C:\Archive\README.md", "README.md");
        assert!(context_score(&a, &c, &w) > context_score(&b, &c, &w));
    }

    /// §24 ordering: SameFolder > Descendant > Ancestor > None.
    #[test]
    fn proximity_ordering() {
        let c = ctx(Some(r"C:\Projects\Launcher"), None);
        let w = ContextRankingWeights::default();
        let same = file_cmd(r"C:\Projects\Launcher\README.md", "r");
        let desc = file_cmd(r"C:\Projects\Launcher\docs\readme.md", "readme");
        // an ancestor candidate is a DIRECTORY above the current folder
        let mut anc = file_cmd(r"C:\Projects", "Projects");
        anc.category = Category::Folder;
        let none = file_cmd(r"D:\Archive\readme.md", "readme");
        let s = |x: &Command| context_score(x, &c, &w);
        assert!(s(&same) > s(&desc));
        assert!(s(&desc) > s(&anc));
        assert!(s(&anc) > s(&none));
    }

    /// CTX-005/§59: foreground exact match boosts the matching app.
    #[test]
    fn foreground_match_boost() {
        let c = ctx(None, Some(r"C:\Program Files\VSCode\code.exe"));
        let w = ContextRankingWeights::default();
        let a = file_cmd(r"C:\Program Files\VSCode\code.exe", "VS Code");
        assert!(context_score(&a, &c, &w) > 0.0);
        let other = file_cmd(r"C:\Chrome\chrome.exe", "Chrome");
        assert_eq!(context_score(&other, &c, &w), 0.0);
    }

    /// §112: no filesystem access — different spellings resolve lexically.
    #[test]
    fn lexical_only_comparison() {
        let c = ctx(Some(r"C:\Projects\Launcher"), None);
        let w = ContextRankingWeights::default();
        let a = file_cmd(r"c:/projects/launcher/x.txt", "x");
        assert!(context_score(&a, &c, &w) > 0.0);
    }
}

#[cfg(test)]
mod merge_actions_tests {
    use super::*;
    use launcher_domain::{Action, ActionKind, Category};

    /// P2.1-C Gate C7: identity collapse merges ACTION DESCRIPTORS — the
    /// canonical row gains the alternative action the dropped row carried.
    #[test]
    fn identity_merge_unions_action_descriptors() {
        let mk = |id: &str, title: &str, actions: Vec<Action>| Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions,
            target: None,
        };
        let open = |id: &str| Action {
            kind: ActionKind::Open,
            payload: None,
            id: Some(id.into()),
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        };
        let q = QueryContext::parse("chrome");
        let mut canonical = mk("a", "Google Chrome", vec![open("open")]);
        canonical.target = Some(r"C:\Chrome\chrome.exe".into());
        canonical.category = Category::Application;
        let mut alt = mk("b", "chrome", vec![open("open"), open("open-private")]);
        alt.target = Some(r"c:/chrome/chrome.EXE".into());
        alt.category = Category::Application;
        alt.actions.push(Action {
            kind: ActionKind::RunAsAdmin,
            payload: None,
            id: Some("runas".into()),
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        });
        let out = rank(vec![canonical, alt], &q, 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].actions.len(), 3, "union of open/open-private/runas");
    }
}
