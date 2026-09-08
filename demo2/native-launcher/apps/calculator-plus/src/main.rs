//! Reference plugin for COMMAND/ACTION-CONTRACT v0.1 (MVP3.0, ADR-0011).
//!
//! Demonstrates ActionDescriptors with primary/secondary semantics:
//!   Copy (= primary, Enter) / Insert (secondary) / Open History (secondary)
//!   plus one intentionally unknown secondary (`plugin.*` namespace) to prove
//!   action-level fault containment: it must be dropped by the host while
//!   the command survives. Only `system.*` actions are used (review 16 §16);
//!   the `plugin.*` execution RPC is out of scope.

use launcher_plugin_api::serve_with_actions;

fn evaluate(input: &str) -> Option<f64> {
    if input.is_empty() {
        return None;
    }
    let tokens: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut pos = 0usize;
    let v = parse_expr(&tokens, &mut pos).ok()?;
    if pos != tokens.len() || !v.is_finite() {
        return None;
    }
    Some(v)
}

// Recursive descent with precedence (ported from calculator-plugin):
// expr := term (('+'|'-') term)* ; term := factor (('*'|'/'|'%') factor)*
// factor := unary ('^' factor)? ; unary := ['-'] atom ; atom := number | '(' expr ')'
fn parse_expr(t: &[char], pos: &mut usize) -> Result<f64, ()> {
    let mut left = parse_term(t, pos)?;
    while *pos < t.len() && matches!(t[*pos], '+' | '-') {
        let op = t[*pos];
        *pos += 1;
        let right = parse_term(t, pos)?;
        left = if op == '+' {
            left + right
        } else {
            left - right
        };
    }
    Ok(left)
}

fn parse_term(t: &[char], pos: &mut usize) -> Result<f64, ()> {
    let mut left = parse_factor(t, pos)?;
    while *pos < t.len() && matches!(t[*pos], '*' | '/' | '%') {
        let op = t[*pos];
        *pos += 1;
        let right = parse_factor(t, pos)?;
        left = match op {
            '*' => left * right,
            '/' if right != 0.0 => left / right,
            '%' if right != 0.0 => left % right,
            _ => return Err(()),
        };
    }
    Ok(left)
}

fn parse_factor(t: &[char], pos: &mut usize) -> Result<f64, ()> {
    let base = parse_unary(t, pos)?;
    if *pos < t.len() && t[*pos] == '^' {
        *pos += 1;
        return Ok(base.powf(parse_factor(t, pos)?));
    }
    Ok(base)
}

fn parse_unary(t: &[char], pos: &mut usize) -> Result<f64, ()> {
    if *pos < t.len() && t[*pos] == '-' {
        *pos += 1;
        return Ok(-parse_unary(t, pos)?);
    }
    parse_atom(t, pos)
}

fn parse_atom(t: &[char], pos: &mut usize) -> Result<f64, ()> {
    if *pos < t.len() && t[*pos] == '(' {
        *pos += 1;
        let v = parse_expr(t, pos)?;
        if *pos >= t.len() || t[*pos] != ')' {
            return Err(());
        }
        *pos += 1;
        return Ok(v);
    }
    let start = *pos;
    while *pos < t.len() && (t[*pos].is_ascii_digit() || t[*pos] == '.') {
        *pos += 1;
    }
    if start == *pos {
        return Err(());
    }
    t[start..*pos]
        .iter()
        .collect::<String>()
        .parse::<f64>()
        .map_err(|_| ())
}

fn descriptor(
    id: &str,
    title: &str,
    ty: &str,
    input: serde_json::Value,
    requires: &[&str],
) -> serde_json::Value {
    let _ = requires; // declared capabilities live in the manifest; requires must stay a subset
    serde_json::json!({
        "id": id,
        "title": title,
        "type": ty,
        "input": input,
        "requires": requires,
    })
}

fn with_extra(mut d: serde_json::Value, fields: serde_json::Value) -> serde_json::Value {
    if let (Some(obj), Some(ext)) = (d.as_object_mut(), fields.as_object()) {
        for (k, v) in ext {
            obj.insert(k.clone(), v.clone());
        }
    }
    d
}

fn handle_query(text: &str) -> serde_json::Value {
    let expr = text.replace('x', "*");
    match evaluate(expr.trim()) {
        Some(v) => {
            let result = if v == v.trunc() {
                format!("{}", v as i64)
            } else {
                format!("{v}")
            };
            serde_json::json!([{
                "title": format!("= {result}"),
                "subtitle": text,
                // bounded ranking hint (COMMAND-CONTRACT section 6): a math
                // result has no lexical relation to the query, so without the
                // hint ranking would drop it entirely
                "score": 1.0,
                "actions": [
                    // MVP3.2-A: shortcut dispatches to the stable action id
                    with_extra(
                        descriptor("copy", "Copy result", "system.copy_to_clipboard",
                                   serde_json::json!({ "text": result }), &["clipboard.write"]),
                        serde_json::json!({ "shortcut": "Ctrl+Shift+C" }),
                    ),
                    descriptor("history", "Open History", "system.open",
                               serde_json::json!({ "target": "history.log" }), &[]),
                    // MVP3.2 effect extension + confirmation demo: injecting a
                    // keystroke into another window is high-risk -> requires
                    // double-Enter confirmation (INV-041)
                    with_extra(
                        descriptor("paste", "Paste at cursor", "system.paste",
                                   serde_json::json!({}), &[]),
                        serde_json::json!({ "confirmation": "confirm" }),
                    ),
                    // unknown `plugin.*` secondary: host must drop it (Hidden)
                    // without invalidating this command (INV-031)
                    // MVP4.0: plugin-owned action (own plugin id, INV-046)
                    descriptor("echo", "Echo via plugin RPC", "plugin.com.example.calculator.plus.echo",
                               serde_json::json!({ "source": text }), &["plugin.invoke"]),
                    descriptor("magic", "Magic", "plugin.com.example.calculator.plus.magic",
                               serde_json::json!({}), &[]),
                ],
            }])
        }
        None => serde_json::json!([{
            "title": "Calculator Plus",
            "subtitle": "type an expression, e.g. 12+34*2",
            "actions": []
        }]),
    }
}

fn main() {
    serve_with_actions(
        handle_query,
        |action_id, input, _generation| match action_id {
            "echo" => Ok(serde_json::json!({
                "echoed": input,
                "by": "calculator-plus",
            })),
            other => Err(format!("unknown action: {other}")),
        },
    )
    .expect("plugin io");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_and_formats() {
        assert_eq!(evaluate("12+34*2"), Some(80.0));
        assert_eq!(evaluate("(1+2)/3"), Some(1.0));
        assert_eq!(evaluate("2^10"), Some(1024.0));
        assert_eq!(evaluate("8/2"), Some(4.0));
        assert_eq!(evaluate(""), None);
        assert_eq!(evaluate("1/0"), None);
    }
}
