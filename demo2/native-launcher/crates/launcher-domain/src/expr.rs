//! Workflow v0.2 expression + value model (MVP4.4 P1-C, review 58 §4/§11-
//! §14): a tiny typed expression language. NOT a scripting runtime:
//! literals, variable references, comparisons, string predicates, logical
//! operators, parentheses. Strictly typed (no coercion), deterministic,
//! pure — evaluation touches nothing but the values it is given (§46/§47).

use std::collections::BTreeMap;

use thiserror::Error;
use std::fmt;

/// Workflow value domain (review 58 §4): JSON-shaped data only. Objects
/// like Effect/Runtime/Capability can never enter a workflow variable.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    pub fn from_json(v: &serde_json::Value) -> Value {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(a) => {
                Value::Array(a.iter().map(Value::from_json).collect())
            }
            serde_json::Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, v)| (k.clone(), Value::from_json(v)))
                    .collect(),
            ),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(n) => serde_json::Value::Number(
                serde_json::Number::from_f64(*n).unwrap_or(0.into()),
            ),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Array(a) => serde_json::Value::Array(a.iter().map(Value::to_json).collect()),
            Value::Object(o) => serde_json::Value::Object(
                o.iter().map(|(k, v)| (k.clone(), v.to_json())).collect(),
            ),
        }
    }
}

/// Expression AST (review 58 §12). Depth is bounded by the parser.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    /// Namespaced reference: `var.x`, `input.y`, `step.id`, `workflow.id`.
    Ref(String),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Contains(Box<Expr>, Box<Expr>),
    StartsWith(Box<Expr>, Box<Expr>),
    EndsWith(Box<Expr>, Box<Expr>),
    Exists(Box<Expr>),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExprError {
    #[error("expression parse error: {0}")]
    Parse(String),
    #[error("expression too deep (max {0})")]
    TooDeep(usize),
    #[error("type mismatch: {left} vs {right}")]
    TypeMismatch { left: &'static str, right: &'static str },
    #[error("unknown variable reference: {0}")]
    UnknownRef(String),
    #[error("output path not found: {0}")]
    OutputPathNotFound(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Number(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "{s}"),
            _ => f.write_str("[value]"),
        }
    }
}

// ---------------- parser ----------------

const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),   // bare word: and/or/not/contains/starts_with/.../true/false
    Ref(String),     // var.x / input.y / step.z / workflow.z
    Str(String),     // "..."
    Number(f64),
    Sym(&'static str), // == != <= >= < > ( )
}

fn tokenize(src: &str) -> Result<Vec<Tok>, ExprError> {
    let mut toks = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '(' => {
                toks.push(Tok::Sym("("));
                chars.next();
            }
            ')' => {
                toks.push(Tok::Sym(")"));
                chars.next();
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    toks.push(Tok::Sym("=="));
                } else {
                    return Err(ExprError::Parse("expected ==".into()));
                }
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    toks.push(Tok::Sym("!="));
                } else {
                    return Err(ExprError::Parse("expected !=".into()));
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    toks.push(Tok::Sym("<="));
                } else {
                    toks.push(Tok::Sym("<"));
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    toks.push(Tok::Sym(">="));
                } else {
                    toks.push(Tok::Sym(">"));
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                while let Some(&c2) = chars.peek() {
                    chars.next();
                    if c2 == '"' {
                        break;
                    }
                    s.push(c2);
                }
                toks.push(Tok::Str(s));
            }
            c if c.is_ascii_digit() => {
                let mut s = String::new();
                while let Some(&c2) = chars.peek() {
                    if c2.is_ascii_digit() || c2 == '.' {
                        s.push(c2);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let n: f64 = s.parse().map_err(|_| ExprError::Parse(format!("bad number {s}")))?;
                toks.push(Tok::Number(n));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut s = String::new();
                while let Some(&c2) = chars.peek() {
                    if c2.is_alphanumeric() || c2 == '_' || c2 == '.' {
                        s.push(c2);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if s.contains('.') {
                    toks.push(Tok::Ref(s));
                } else {
                    toks.push(Tok::Ident(s));
                }
            }
            other => {
                return Err(ExprError::Parse(format!("unexpected character {other}")));
            }
        }
    }
    Ok(toks)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn expr(&mut self) -> Result<Expr, ExprError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ExprError::TooDeep(MAX_DEPTH));
        }
        let v = self.or_expr();
        self.depth -= 1;
        v
    }

    fn or_expr(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.and_expr()?;
        while self.peek() == Some(&Tok::Ident("or".into())) {
            self.pos += 1;
            let right = self.and_expr()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and_expr(&mut self) -> Result<Expr, ExprError> {
        let mut left = self.not_expr()?;
        while self.peek() == Some(&Tok::Ident("and".into())) {
            self.pos += 1;
            let right = self.not_expr()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn not_expr(&mut self) -> Result<Expr, ExprError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ExprError::TooDeep(MAX_DEPTH));
        }
        let v = self.not_expr_inner();
        self.depth -= 1;
        v
    }

    fn not_expr_inner(&mut self) -> Result<Expr, ExprError> {
        if self.peek() == Some(&Tok::Ident("not".into())) {
            self.pos += 1;
            let inner = self.not_expr()?;
            return Ok(Expr::Not(Box::new(inner)));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expr, ExprError> {
        // `exists` binds a reference directly: `var.x exists`
        let left = self.primary()?;
        let op = match self.peek() {
            Some(Tok::Sym("==")) => "eq",
            Some(Tok::Sym("!=")) => "ne",
            Some(Tok::Sym("<")) => "lt",
            Some(Tok::Sym("<=")) => "le",
            Some(Tok::Sym(">")) => "gt",
            Some(Tok::Sym(">=")) => "ge",
            Some(Tok::Ident(w)) if w == "contains" => "contains",
            Some(Tok::Ident(w)) if w == "starts_with" => "starts_with",
            Some(Tok::Ident(w)) if w == "ends_with" => "ends_with",
            Some(Tok::Ident(w)) if w == "exists" => {
                self.pos += 1;
                return Ok(Expr::Exists(Box::new(left)));
            }
            _ => return Ok(left),
        };
        self.pos += 1;
        let right = self.primary()?;
        Ok(match op {
            "eq" => Expr::Eq(Box::new(left), Box::new(right)),
            "ne" => Expr::Ne(Box::new(left), Box::new(right)),
            "lt" => Expr::Lt(Box::new(left), Box::new(right)),
            "le" => Expr::Le(Box::new(left), Box::new(right)),
            "gt" => Expr::Gt(Box::new(left), Box::new(right)),
            "ge" => Expr::Ge(Box::new(left), Box::new(right)),
            "contains" => Expr::Contains(Box::new(left), Box::new(right)),
            "starts_with" => Expr::StartsWith(Box::new(left), Box::new(right)),
            _ => Expr::EndsWith(Box::new(left), Box::new(right)),
        })
    }

    fn primary(&mut self) -> Result<Expr, ExprError> {
        match self.toks.get(self.pos).cloned() {
            Some(Tok::Sym(sym)) if sym == "(" => {
                self.pos += 1;
                let inner = self.expr()?;
                match self.toks.get(self.pos) {
                    Some(Tok::Sym(")")) => {
                        self.pos += 1;
                        Ok(inner)
                    }
                    _ => Err(ExprError::Parse("expected )".into())),
                }
            }
            Some(Tok::Number(n)) => {
                self.pos += 1;
                Ok(Expr::Literal(Value::Number(n)))
            }
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Ok(Expr::Literal(Value::String(s.clone())))
            }
            Some(Tok::Ident(w)) if w == "true" => {
                self.pos += 1;
                Ok(Expr::Literal(Value::Bool(true)))
            }
            Some(Tok::Ident(w)) if w == "false" => {
                self.pos += 1;
                Ok(Expr::Literal(Value::Bool(false)))
            }
            Some(Tok::Ident(w)) if w == "null" => {
                self.pos += 1;
                Ok(Expr::Literal(Value::Null))
            }
            Some(Tok::Ref(r)) => {
                self.pos += 1;
                Ok(Expr::Ref(r.clone()))
            }
            other => Err(ExprError::Parse(format!("unexpected token {other:?}"))),
        }
    }
}

/// Parse a condition expression (review 58 §12 grammar).
pub fn parse_expr(src: &str) -> Result<Expr, ExprError> {
    let mut p = Parser { toks: tokenize(src)?, pos: 0, depth: 0 };
    let e = p.expr()?;
    if p.pos != p.toks.len() {
        return Err(ExprError::Parse("trailing tokens".into()));
    }
    Ok(e)
}

// ---------------- evaluation ----------------

/// Variable resolver: maps a namespaced reference (`var.x`,
/// `input.source`) to its value; None = reference not set. Pure.
pub type RefResolver<'a> = dyn Fn(&str) -> Option<Value> + 'a;

/// Evaluate an expression strictly (review 58 §13): no type coercion —
/// cross-type comparisons are TypeMismatch (equality between different
/// types is simply false for Eq; explicitly false ≠ TypeMismatch per
/// JSON semantics, but ORDERED comparisons across types are TypeMismatch).
pub fn evaluate(expr: &Expr, resolve: &RefResolver<'_>) -> Result<Value, ExprError> {
    let v = eval_inner(expr, resolve)?;
    Ok(v)
}

fn eval_inner(expr: &Expr, resolve: &RefResolver<'_>) -> Result<Value, ExprError> {
    Ok(match expr {
        Expr::Literal(v) => v.clone(),
        Expr::Ref(r) => resolve(r).unwrap_or(Value::Null),
        Expr::Exists(inner) => {
            let resolved = match inner.as_ref() {
                Expr::Ref(path) => resolve(path),
                _ => None,
            };
            Value::Bool(resolved.is_some())
        }
        Expr::Not(inner) => {
            let v = expect_bool(&eval_inner(inner, resolve)?)?;
            Value::Bool(!v)
        }
        Expr::And(a, b) => {
            let va = expect_bool(&eval_inner(a, resolve)?)?;
            let vb = expect_bool(&eval_inner(b, resolve)?)?;
            Value::Bool(va && vb)
        }
        Expr::Or(a, b) => {
            let va = expect_bool(&eval_inner(a, resolve)?)?;
            let vb = expect_bool(&eval_inner(b, resolve)?)?;
            Value::Bool(va || vb)
        }
        Expr::Eq(a, b) => {
            let (va, vb) = (eval_inner(a, resolve)?, eval_inner(b, resolve)?);
            Value::Bool(values_equal(&va, &vb))
        }
        Expr::Ne(a, b) => {
            let (va, vb) = (eval_inner(a, resolve)?, eval_inner(b, resolve)?);
            Value::Bool(!values_equal(&va, &vb))
        }
        Expr::Lt(a, b) | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) => {
            let (va, vb) = (eval_inner(a, resolve)?, eval_inner(b, resolve)?);
            let ord = ordered(&va, &vb)?;
            let lt = matches!(expr, Expr::Lt(_, _));
            let le = matches!(expr, Expr::Le(_, _));
            let gt = matches!(expr, Expr::Gt(_, _));
            Value::Bool(if lt {
                ord == std::cmp::Ordering::Less
            } else if le {
                ord != std::cmp::Ordering::Greater
            } else if gt {
                ord == std::cmp::Ordering::Greater
            } else {
                ord != std::cmp::Ordering::Less
            })
        }
        Expr::Contains(a, b) => {
            let (va, vb) = (eval_inner(a, resolve)?, eval_inner(b, resolve)?);
            match (&va, &vb) {
                (Value::String(h), Value::String(n)) => Value::Bool(h.contains(n.as_str())),
                (Value::Array(h), other) => {
                    Value::Bool(h.iter().any(|item| values_equal(item, other)))
                }
                (Value::Null, _) | (_, Value::Null) => Value::Bool(false),
                _ => {
                    return Err(ExprError::TypeMismatch {
                        left: va.type_name(),
                        right: vb.type_name(),
                    })
                }
            }
        }
        Expr::StartsWith(a, b) | Expr::EndsWith(a, b) => {
            let (va, vb) = (eval_inner(a, resolve)?, eval_inner(b, resolve)?);
            match (&va, &vb) {
                (Value::String(h), Value::String(n)) => {
                    let starts = matches!(expr, Expr::StartsWith(_, _));
                    Value::Bool(if starts {
                        h.starts_with(n.as_str())
                    } else {
                        h.ends_with(n.as_str())
                    })
                }
                _ => {
                    return Err(ExprError::TypeMismatch {
                        left: va.type_name(),
                        right: vb.type_name(),
                    })
                }
            }
        }
    })
}

fn expect_bool(v: &Value) -> Result<bool, ExprError> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(ExprError::TypeMismatch {
            left: other.type_name(),
            right: "bool",
        }),
    }
}

/// Eq across different types = false (JSON semantics); ordered comparison
/// across types = TypeMismatch (§13).
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| y.get(k).map(|bv| values_equal(v, bv)).unwrap_or(false))
        }
        _ => false,
    }
}

fn ordered(a: &Value, b: &Value) -> Result<std::cmp::Ordering, ExprError> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x.partial_cmp(y).ok_or(ExprError::TypeMismatch {
                left: "number",
                right: "number",
            })
        }
        (Value::String(x), Value::String(y)) => Ok(x.cmp(y)),
        _ => Err(ExprError::TypeMismatch {
            left: a.type_name(),
            right: b.type_name(),
        }),
    }
}

/// Walk a nested object path ("a.b.c") inside a Value; missing segment =
/// None (callers distinguish UnknownRef vs OutputPathNotFound).
pub fn walk_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path.split('.') {
        match cur {
            Value::Object(o) => cur = o.get(seg)?,
            Value::Array(a) => {
                let idx: usize = seg.parse().ok()?;
                cur = a.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

/// Typed BTreeMap re-export for object values (§4).
pub type ObjectMap = BTreeMap<String, Value>;
