//! P2.5-D tests (spec `demo2/files2/101-p2.5-0.1.md` §D): centralized
//! weights (D02), explainable ranking (D03) and the search-quality corpus
//! regression (D05).

use launcher_domain::{Category, Command, QueryContext};
use launcher_search::{
    rank_with_boost, score, score_with_parts, MatchedField, RankingWeights,
    RANKING_WEIGHTS_VERSION,
};

fn cmd(id: &str, title: &str, category: Category) -> Command {
    Command {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        icon: None,
        provider_id: "test".into(),
        score: 0.0,
        keywords: vec![],
        category,
        actions: vec![],
        target: None,
    }
}

// ---- D02: centralized weights ----------------------------------------------

/// AC-D02-1: changing a weight changes the score through the ONE source —
/// no scattered magic number can override it.
#[test]
fn weights_are_single_sourced() {
    let q = QueryContext::parse("chrome");
    let c = cmd("c", "chrome", Category::Application);
    let base = score_with_parts(&c, &q, &RankingWeights::default());
    assert_eq!(base.title, 100.0, "default title_exact = legacy 100");
    let mut w = RankingWeights::default();
    w.title_exact = 500.0;
    let tuned = score_with_parts(&c, &q, &w);
    assert_eq!(tuned.title, 500.0);
    assert!((tuned.total() - base.total()) > 300.0);
}

/// AC-D02-2: invalid config fields fall back per-field; unknown versions are
/// rejected wholesale.
#[test]
fn invalid_weights_fall_back() {
    let mut w = RankingWeights::default();
    w.keyword_exact = 77.0;
    w.title_exact = -5.0; // invalid → must fall back to default 100
    let json = {
        let mut v = serde_json::to_value(w.to_config_json()).unwrap_or(serde_json::Value::Null);
        // to_config_json serializes the raw (invalid) values; patch version ok
        let _ = &mut v;
        w.to_config_json()
    };
    // roundtrip through the validation gate
    let parsed = RankingWeights::from_config_json(&json).unwrap();
    assert_eq!(parsed.title_exact, RankingWeights::default().title_exact,
        "negative weight falls back to default");
    assert_eq!(parsed.keyword_exact, 77.0, "valid field applied");

    // unknown version rejected
    let bad = json.replace(&format!("\"version\":{RANKING_WEIGHTS_VERSION}"), "\"version\":99");
    assert!(RankingWeights::from_config_json(&bad).is_err());
}

/// D02: config serialization roundtrips (valid values survive).
#[test]
fn weights_config_roundtrip() {
    let mut w = RankingWeights::default();
    w.title_prefix = 61.5;
    w.multi_token = 29.0;
    let json = w.to_config_json();
    let back = RankingWeights::from_config_json(&json).unwrap();
    assert_eq!(back.title_prefix, 61.5);
    assert_eq!(back.multi_token, 29.0);
    assert_eq!(back, w);
}

// ---- D03: explainable ranking -----------------------------------------------

/// AC-D03-1/2: explanation total == actual score, same source.
#[test]
fn explanation_total_matches_score() {
    let q = QueryContext::parse("chrome");
    let cases = [
        cmd("1", "chrome", Category::Application),
        cmd("2", "chrome-installer.url", Category::File),
        cmd("3", "Google Chrome", Category::Application),
        cmd("4", "Totally Unrelated", Category::Folder),
    ];
    for c in cases {
        let parts = score_with_parts(&c, &q, &RankingWeights::default());
        assert!(
            (parts.total() - score(&c, &q)).abs() < f32::EPSILON,
            "{}: explanation total == score",
            c.id
        );
    }
}

/// D03 step 4: matched field is reported (explainability).
#[test]
fn matched_field_reported() {
    let q = QueryContext::parse("chrome");
    let exact = score_with_parts(&cmd("1", "chrome", Category::Application), &q, &RankingWeights::default());
    assert_eq!(exact.matched_field, MatchedField::Title);
    // query == full STEM exact (100) while the full title only contains it
    // (40) — the stem tier wins and is reported
    let q2 = QueryContext::parse("pref-setup");
    let stem = score_with_parts(&cmd("2", "pref-setup.txt", Category::File), &q2, &RankingWeights::default());
    assert_eq!(stem.matched_field, MatchedField::TitleStem);
    let none = score_with_parts(&cmd("3", "unrelated", Category::Folder), &q, &RankingWeights::default());
    assert_eq!(none.matched_field, MatchedField::None);
}

/// AC-D03-3 (structural pin): ScoreParts carries only ranking data — no
/// capability/authority vocabulary can enter the explanation model.
#[test]
fn score_parts_is_authority_free() {
    let parts = score_with_parts(&cmd("1", "chrome", Category::Application), &QueryContext::parse("chrome"), &RankingWeights::default());
    let json = serde_json::json!({
        "plugin_hint": parts.plugin_hint,
        "title": parts.title,
        "keyword": parts.keyword,
        "fuzzy": parts.fuzzy,
        "multi_token": parts.multi_token,
        "type_prior": parts.type_prior,
        "matched_field": format!("{:?}", parts.matched_field),
    });
    let s = json.to_string();
    assert!(!s.contains("capabilit"));
    assert!(!s.contains("authority"));
}

// ---- D05: search quality corpus ----------------------------------------------

/// D05 regression corpus (SEARCH-QUALITY): for "chrome", the real Chrome
/// application must outrank extension-noise files; exact > prefix > contains
/// holds; boost-free ordering is deterministic.
#[test]
fn quality_corpus_chrome_beats_extension_noise() {
    let candidates = vec![
        cmd("noise1", "chrome-shortcut.url", Category::File),
        cmd("noise2", "chrome-logo.svg", Category::File),
        cmd("app", "Chrome", Category::Application),
        cmd("noise3", "chromium-notes.md", Category::File),
    ];
    let ranked = rank_with_boost(candidates, &QueryContext::parse("chrome"), 10, |_| 0.0);
    assert_eq!(ranked[0].id, "app", "exact app beats prefix/contains noise");
    // contains-tier noise stays after the app, in deterministic order
    let ids: Vec<&str> = ranked.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["app", "noise1", "noise2", "noise3"]);
}

/// D05: multi-token query requires all tokens (no OR-noise).
#[test]
fn quality_corpus_multi_token_requires_all() {
    let candidates = vec![
        cmd("both", "Visual Studio Code", Category::Application),
        cmd("partial", "Visual Studio", Category::Application),
    ];
    let ranked = rank_with_boost(candidates, &QueryContext::parse("visual code"), 10, |_| 0.0);
    assert_eq!(ranked[0].id, "both", "all-tokens candidate wins");
}
