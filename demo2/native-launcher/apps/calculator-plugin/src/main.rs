//! Reference plugin (PLUGIN-014): "Calculator Extended".
//!
//! Evaluates arithmetic queries (`12+34*2`, `(1+2)/3`, `2^10`) against the
//! frozen Plugin Contract v0.1: manifest v2 schema, initialize/shutdown
//! handshake, query_id echo, zero capabilities. Written on top of
//! `launcher-plugin-api::serve`, so the developer never touches JSON-RPC.

use launcher_plugin_api::serve;

/// Evaluate a basic arithmetic expression (recursive descent).
/// Grammar: expr := term (('+'|'-') term)* ; term := factor (('*'|'/'|'%') factor)*
/// factor := unary ('^' factor)? ; unary := ['-'] atom ; atom := number | '(' expr ')'
pub fn evaluate(input: &str) -> Result<f64, String> {
    let tokens: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
    let mut pos = 0;
    let v = expr(&tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err(format!("unexpected character at {}", pos + 1));
    }
    if v.is_nan() || v.is_infinite() {
        return Err("result is not finite".into());
    }
    Ok(v)
}

fn expr(t: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = term(t, pos)?;
    while *pos < t.len() && matches!(t[*pos], '+' | '-') {
        let op = t[*pos];
        *pos += 1;
        let right = term(t, pos)?;
        left = if op == '+' {
            left + right
        } else {
            left - right
        };
    }
    Ok(left)
}

fn term(t: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = factor(t, pos)?;
    while *pos < t.len() && matches!(t[*pos], '*' | '/' | '%') {
        let op = t[*pos];
        *pos += 1;
        let right = factor(t, pos)?;
        left = match op {
            '*' => left * right,
            '/' if right == 0.0 => return Err("division by zero".into()),
            '/' => left / right,
            _ => left % right,
        };
    }
    Ok(left)
}

fn factor(t: &[char], pos: &mut usize) -> Result<f64, String> {
    let base = unary(t, pos)?;
    if *pos < t.len() && t[*pos] == '^' {
        *pos += 1;
        let exp = factor(t, pos)?; // right-associative
        return Ok(base.powf(exp));
    }
    Ok(base)
}

fn unary(t: &[char], pos: &mut usize) -> Result<f64, String> {
    if *pos < t.len() && t[*pos] == '-' {
        *pos += 1;
        return Ok(-unary(t, pos)?);
    }
    atom(t, pos)
}

fn atom(t: &[char], pos: &mut usize) -> Result<f64, String> {
    if *pos < t.len() && t[*pos] == '(' {
        *pos += 1;
        let v = expr(t, pos)?;
        if *pos >= t.len() || t[*pos] != ')' {
            return Err("missing closing parenthesis".into());
        }
        *pos += 1;
        return Ok(v);
    }
    let start = *pos;
    while *pos < t.len() && (t[*pos].is_ascii_digit() || t[*pos] == '.') {
        *pos += 1;
    }
    if start == *pos {
        return Err(format!("expected a number at {}", start + 1));
    }
    t[start..*pos]
        .iter()
        .collect::<String>()
        .parse::<f64>()
        .map_err(|e| e.to_string())
}

fn format_result(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Contract query handler: arithmetic-looking input evaluates, anything else
/// yields one help item so the query always returns something discoverable.
fn handle_query(text: &str) -> serde_json::Value {
    let text = text.trim();
    let looks_like_math = text.chars().any(|c| c.is_ascii_digit())
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || "+-*/%^(). ".contains(c) || c == 'x' || c == ',');
    if looks_like_math && !text.is_empty() {
        // allow "x" as multiplication alias: "2x3"
        let normalized = text.replace('x', "*").replace(',', ".");
        match evaluate(&normalized) {
            Ok(v) => {
                return serde_json::json!([
                    {
                        "title": format!("= {}", format_result(v)),
                        "subtitle": text,
                        "actions": ["copy"]
                    }
                ]);
            }
            Err(e) => {
                return serde_json::json!([
                    { "title": format!("Cannot calculate: {e}"), "subtitle": text, "actions": [] }
                ]);
            }
        }
    }
    serde_json::json!([
        {
            "title": "Calculator Extended",
            "subtitle": "type an expression, e.g. 12+34*2 or (1+2)/3 or 2^10",
            "actions": []
        }
    ])
}

fn main() {
    serve(handle_query).expect("plugin io");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_basic_arithmetic() {
        assert_eq!(evaluate("12+34*2").unwrap(), 80.0);
        assert_eq!(evaluate("(1+2)/3").unwrap(), 1.0);
        assert_eq!(evaluate("2^10").unwrap(), 1024.0);
        assert!(evaluate("2x3").is_err()); // x is handled above evaluate
        assert_eq!(evaluate("-5+3").unwrap(), -2.0);
        assert_eq!(evaluate(" 7 % 3 ").unwrap(), 1.0);
        assert_eq!(evaluate("2^3^2").unwrap(), 512.0); // right-associative
    }

    #[test]
    fn rejects_bad_input_without_panicking() {
        assert!(evaluate("").is_err());
        assert!(evaluate("1/0").is_err());
        assert!(evaluate("(1+2").is_err());
        assert!(evaluate("abc").is_err());
        assert!(evaluate("1..2").is_err());
    }

    #[test]
    fn query_handler_returns_contract_items() {
        let v = handle_query("12+34*2");
        assert_eq!(v[0]["title"], "= 80");

        let v = handle_query("hello");
        assert!(v[0]["title"].as_str().unwrap().contains("Calculator"));

        let v = handle_query("1/0");
        assert!(v[0]["title"].as_str().unwrap().contains("Cannot"));
    }
}
