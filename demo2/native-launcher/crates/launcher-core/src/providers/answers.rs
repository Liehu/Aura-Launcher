//! Instant Answers provider (P3.0-F03): arithmetic expressions typed into
//! the search box are evaluated inline — `=1+2*3` or a bare pure-arithmetic
//! query like `1+2*3` surfaces an Answer row whose Enter action copies the
//! result.
//!
//! Security boundary (UX-R2): the evaluator is a small pure recursive-
//! descent arithmetic parser — numbers, `+ - * / %`, parentheses, f64.
//! No variables, no functions, no IO. Parse errors, division by zero,
//! overflow and depth overflow all degrade to "no answer row" (the query
//! falls through to normal search).

use launcher_domain::{Action, ActionKind, ActionPayload, Category, Command, QueryContext};

pub const PROVIDER_ID: &str = "answers";

const MAX_DEPTH: usize = 32;

/// Evaluate a pure arithmetic expression. A leading `=` is optional and
/// stripped. `None` = not an answer.
pub fn evaluate_arithmetic(src: &str) -> Option<f64> {
    let src = src.trim();
    let src = src.strip_prefix('=').unwrap_or(src);
    let tokens: Vec<char> = src.chars().filter(|c| !c.is_whitespace()).collect();
    if tokens.is_empty() {
        return None;
    }
    let mut pos = 0usize;
    let v = parse_expr(&tokens, &mut pos, 0)?;
    if pos != tokens.len() {
        return None; // trailing garbage — not an answer
    }
    if !v.is_finite() {
        return None; // div by zero → inf/nan → degrade
    }
    Some(v)
}

fn parse_expr(tokens: &[char], pos: &mut usize, depth: usize) -> Option<f64> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut lhs = parse_term(tokens, pos, depth)?;
    while matches!(tokens.get(*pos), Some('+') | Some('-')) {
        let op = tokens[*pos];
        *pos += 1;
        let rhs = parse_term(tokens, pos, depth)?;
        lhs = if op == '+' { lhs + rhs } else { lhs - rhs };
    }
    Some(lhs)
}

fn parse_term(tokens: &[char], pos: &mut usize, depth: usize) -> Option<f64> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut lhs = parse_factor(tokens, pos, depth)?;
    while matches!(tokens.get(*pos), Some('*') | Some('/') | Some('%')) {
        let op = tokens[*pos];
        *pos += 1;
        let rhs = parse_factor(tokens, pos, depth)?;
        lhs = match op {
            '*' => lhs * rhs,
            '/' if rhs != 0.0 => lhs / rhs,
            '%' if rhs != 0.0 => lhs % rhs,
            _ => return None, // division/mod by zero → not an answer
        };
    }
    Some(lhs)
}

fn parse_factor(tokens: &[char], pos: &mut usize, depth: usize) -> Option<f64> {
    if depth > MAX_DEPTH {
        return None;
    }
    match tokens.get(*pos)? {
        '(' => {
            *pos += 1;
            let v = parse_expr(tokens, pos, depth + 1)?;
            if tokens.get(*pos) != Some(&')') {
                return None;
            }
            *pos += 1;
            Some(v)
        }
        '-' => {
            *pos += 1;
            Some(-parse_factor(tokens, pos, depth + 1)?)
        }
        '+' => {
            *pos += 1;
            parse_factor(tokens, pos, depth + 1)
        }
        c if c.is_ascii_digit() || *c == '.' => parse_number(tokens, pos),
        _ => None,
    }
}

fn parse_number(tokens: &[char], pos: &mut usize) -> Option<f64> {
    let start = *pos;
    while matches!(tokens.get(*pos), Some(c) if c.is_ascii_digit() || *c == '.') {
        *pos += 1;
    }
    if start == *pos {
        return None;
    }
    let s: String = tokens[start..*pos].iter().collect();
    // reject multi-dot garbage ("1.2.3" parses to None via strict parse)
    s.parse::<f64>().ok().filter(|_| s.matches('.').count() <= 1)
}

/// Does this raw query look like an answer request?
/// `=…` (explicit) or a bare pure-arithmetic body (digits + operators only,
/// at least one digit and one operator).
fn is_answer_query(raw: &str) -> Option<&str> {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix('=') {
        return Some(rest);
    }
    let has_digit = t.chars().any(|c| c.is_ascii_digit());
    let has_op = t.chars().any(|c| matches!(c, '+' | '-' | '*' | '/' | '%'));
    let all_allowed = t
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || matches!(c, '+' | '-' | '*' | '/' | '%' | '(' | ')'))
        && !t.chars().any(|c| c.is_whitespace());
    if has_digit && has_op && all_allowed {
        Some(t)
    } else {
        None
    }
}

/// The launcher_domain::Provider implementation: one Answer command when
/// the query evaluates, otherwise nothing.
pub struct AnswersProvider;

impl crate::Provider for AnswersProvider {
    fn id(&self) -> &str {
        PROVIDER_ID
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        query_answers(q)
    }
}

fn query_answers(q: &QueryContext) -> Vec<Command> {
    let Some(body) = is_answer_query(&q.raw) else {
        return vec![];
    };
    let Some(result) = evaluate_arithmetic(body) else {
        return vec![];
    };
    let expr_text: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let result_text = format_number(result);
    // P3.2: a proper calculator page for the detail pane (strong result
    // line + key/value breakdown) instead of raw id diagnostics
    launcher_domain::rich::store(
        "answer:calc",
        launcher_domain::rich::RichResult {
            blocks: vec![
                launcher_domain::rich::RichBlock::Text {
                    text: format!("= {result_text}"),
                    emphasis: launcher_domain::rich::Emphasis::Strong,
                },
                launcher_domain::rich::RichBlock::KeyValue {
                    rows: vec![
                        launcher_domain::rich::KeyValueRow {
                            key: "expression".into(),
                            value: expr_text.clone(),
                        },
                        launcher_domain::rich::KeyValueRow {
                            key: "result".into(),
                            value: result_text.clone(),
                        },
                    ],
                },
                launcher_domain::rich::RichBlock::Divider,
                launcher_domain::rich::RichBlock::Text {
                    text: "Enter copies the result".into(),
                    emphasis: launcher_domain::rich::Emphasis::None,
                },
            ],
        },
    );
    vec![Command {
        id: "answer:calc".into(),
        title: format!("{expr_text} = {result_text}"),
        subtitle: Some("Enter to copy the result".into()),
        icon: None,
        provider_id: PROVIDER_ID.into(),
        // top priority: the Answer row is always first (design §F03)
        score: 1.0,
        keywords: vec![],
        category: Category::Command,
        actions: vec![Action {
            kind: ActionKind::Copy,
            payload: Some(ActionPayload::Text(result_text)),
            id: Some("copy".into()),
            title: Some("Copy result".into()),
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }],
        target: None,
    }]
}

fn format_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ANS-P1: precedence and explicit prefix.
    #[test]
    fn precedence_and_prefix() {
        assert_eq!(evaluate_arithmetic("1+2*3"), Some(7.0));
        assert_eq!(evaluate_arithmetic("=1+2*3"), Some(7.0));
        assert_eq!(evaluate_arithmetic("(1+2)*3"), Some(9.0));
    }

    /// ANS-P2: identifiers/functions/unknown symbols degrade.
    #[test]
    fn unsafe_or_unknown_degrades() {
        assert_eq!(evaluate_arithmetic("std::env"), None);
        assert_eq!(evaluate_arithmetic("foo(1)"), None);
        assert_eq!(evaluate_arithmetic("1 + a"), None);
        assert_eq!(evaluate_arithmetic("1; rm -rf"), None);
    }

    /// ANS-P3: division by zero / trailing garbage degrade.
    #[test]
    fn degenerate_inputs() {
        assert_eq!(evaluate_arithmetic("1/0"), None);
        assert_eq!(evaluate_arithmetic("1+(2"), None);
        assert_eq!(evaluate_arithmetic("1+2)"), None);
        assert_eq!(evaluate_arithmetic("1.2.3"), None);
    }

    /// The provider surfaces exactly one top-priority Copy command.
    #[test]
    fn provider_shape() {
        let q = QueryContext::parse("=6/3");
        let cmds = query_answers(&q);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].title, "6/3 = 2");
        assert_eq!(cmds[0].score, 1.0);
        // non-answer query → nothing
        assert!(query_answers(&QueryContext::parse("chrome")).is_empty());
    }
}
