//! Tiny expression evaluator for policy rule `when` strings.
//!
//! The grammar covers the rule patterns used by Mandate's reference policy and
//! by `docs/spec/16_demo_acceptance.md`:
//!
//! ```text
//! expr        := or_expr
//! or_expr     := and_expr ('or' and_expr)*
//! and_expr    := not_expr ('and' not_expr)*
//! not_expr    := 'not' not_expr | comparison
//! comparison  := primary (cmp_op primary)?
//! cmp_op      := '==' | '!=' | '<=' | '>=' | '<' | '>'
//! primary     := literal | path | '(' expr ')'
//! literal     := bool | number | string | 'null'
//! path        := IDENT ('.' IDENT)*
//! ```
//!
//! The evaluator is **not** a full Rego implementation. The interface contract
//! (`docs/spec/17_interface_contracts.md`) names `regorus` for production; this
//! module is the hackathon-grade replacement covering the locked corpus.

use std::fmt;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExprError {
    #[error("tokenize: unexpected character {0:?} at offset {1}")]
    UnexpectedChar(char, usize),
    #[error("parse: {0}")]
    Parse(String),
    #[error("eval: type mismatch — {0}")]
    TypeMismatch(String),
    #[error("eval: missing path {0}")]
    MissingPath(String),
    #[error("eval: expression must evaluate to bool")]
    NotBool,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    String(String),
    Bool(bool),
    Null,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    Dot,
    LParen,
    RParen,
    And,
    Or,
    Not,
    Eof,
}

fn tokenize(input: &str) -> Result<Vec<Token>, ExprError> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '.' => {
                out.push(Token::Dot);
                i += 1;
            }
            '(' => {
                out.push(Token::LParen);
                i += 1;
            }
            ')' => {
                out.push(Token::RParen);
                i += 1;
            }
            '=' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                out.push(Token::Eq);
                i += 2;
            }
            '!' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                out.push(Token::Neq);
                i += 2;
            }
            '<' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                out.push(Token::Lte);
                i += 2;
            }
            '>' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                out.push(Token::Gte);
                i += 2;
            }
            '<' => {
                out.push(Token::Lt);
                i += 1;
            }
            '>' => {
                out.push(Token::Gt);
                i += 1;
            }
            '"' => {
                let mut j = i + 1;
                let mut s = String::new();
                while j < chars.len() && chars[j] != '"' {
                    if chars[j] == '\\' && j + 1 < chars.len() {
                        s.push(chars[j + 1]);
                        j += 2;
                    } else {
                        s.push(chars[j]);
                        j += 1;
                    }
                }
                if j >= chars.len() {
                    return Err(ExprError::Parse("unterminated string".into()));
                }
                out.push(Token::String(s));
                i = j + 1;
            }
            c if c.is_ascii_digit()
                || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) =>
            {
                let mut j = i;
                if chars[j] == '-' {
                    j += 1;
                }
                while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                    j += 1;
                }
                let s: String = chars[i..j].iter().collect();
                let n: f64 = s
                    .parse()
                    .map_err(|_| ExprError::Parse(format!("bad number {s:?}")))?;
                out.push(Token::Number(n));
                i = j;
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut j = i;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let s: String = chars[i..j].iter().collect();
                let tok = match s.as_str() {
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "true" => Token::Bool(true),
                    "false" => Token::Bool(false),
                    "null" => Token::Null,
                    _ => Token::Ident(s),
                };
                out.push(tok);
                i = j;
            }
            _ => return Err(ExprError::UnexpectedChar(c, i)),
        }
    }
    out.push(Token::Eof);
    Ok(out)
}

#[derive(Debug, Clone)]
enum Node {
    Bool(bool),
    Number(f64),
    String(String),
    Null,
    Path(Vec<String>),
    Cmp(Box<Node>, CmpOp, Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Not(Box<Node>),
}

#[derive(Debug, Clone, Copy)]
enum CmpOp {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        self.pos += 1;
        t
    }

    fn parse_expr(&mut self) -> Result<Node, ExprError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Node, ExprError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Node::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Node, ExprError> {
        let mut left = self.parse_not()?;
        while matches!(self.peek(), Token::And) {
            self.advance();
            let right = self.parse_not()?;
            left = Node::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Node, ExprError> {
        if matches!(self.peek(), Token::Not) {
            self.advance();
            let inner = self.parse_not()?;
            return Ok(Node::Not(Box::new(inner)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Node, ExprError> {
        let left = self.parse_primary()?;
        let op = match self.peek() {
            Token::Eq => Some(CmpOp::Eq),
            Token::Neq => Some(CmpOp::Neq),
            Token::Lt => Some(CmpOp::Lt),
            Token::Lte => Some(CmpOp::Lte),
            Token::Gt => Some(CmpOp::Gt),
            Token::Gte => Some(CmpOp::Gte),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let right = self.parse_primary()?;
            return Ok(Node::Cmp(Box::new(left), op, Box::new(right)));
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Node, ExprError> {
        match self.advance() {
            Token::Bool(b) => Ok(Node::Bool(b)),
            Token::Number(n) => Ok(Node::Number(n)),
            Token::String(s) => Ok(Node::String(s)),
            Token::Null => Ok(Node::Null),
            Token::Ident(first) => {
                let mut parts = vec![first];
                while matches!(self.peek(), Token::Dot) {
                    self.advance();
                    match self.advance() {
                        Token::Ident(n) => parts.push(n),
                        other => {
                            return Err(ExprError::Parse(format!(
                                "expected ident after '.', got {other:?}"
                            )))
                        }
                    }
                }
                Ok(Node::Path(parts))
            }
            Token::LParen => {
                let inner = self.parse_expr()?;
                match self.advance() {
                    Token::RParen => Ok(inner),
                    other => Err(ExprError::Parse(format!("expected ')', got {other:?}"))),
                }
            }
            other => Err(ExprError::Parse(format!("unexpected token {other:?}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Bool(bool),
    Number(f64),
    String(String),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Number(n) => write!(f, "{n}"),
            Value::String(s) => write!(f, "{s:?}"),
            Value::Null => write!(f, "null"),
        }
    }
}

fn lookup(ctx: &serde_json::Value, path: &[String]) -> Result<Value, ExprError> {
    let mut cur = ctx;
    for p in path {
        cur = cur
            .get(p)
            .ok_or_else(|| ExprError::MissingPath(path.join(".")))?;
    }
    json_to_value(cur)
}

fn json_to_value(v: &serde_json::Value) -> Result<Value, ExprError> {
    match v {
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(Value::Number)
            .ok_or_else(|| ExprError::TypeMismatch("number not representable as f64".into())),
        serde_json::Value::String(s) => Ok(Value::String(s.clone())),
        serde_json::Value::Null => Ok(Value::Null),
        _ => Err(ExprError::TypeMismatch(
            "expected scalar (bool/number/string/null)".into(),
        )),
    }
}

fn cmp(a: &Value, op: CmpOp, b: &Value) -> Result<bool, ExprError> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Ok(match op {
            CmpOp::Eq => x == y,
            CmpOp::Neq => x != y,
            CmpOp::Lt => x < y,
            CmpOp::Lte => x <= y,
            CmpOp::Gt => x > y,
            CmpOp::Gte => x >= y,
        }),
        (Value::String(x), Value::String(y)) => Ok(match op {
            CmpOp::Eq => x == y,
            CmpOp::Neq => x != y,
            CmpOp::Lt => x < y,
            CmpOp::Lte => x <= y,
            CmpOp::Gt => x > y,
            CmpOp::Gte => x >= y,
        }),
        (Value::Bool(x), Value::Bool(y)) => match op {
            CmpOp::Eq => Ok(x == y),
            CmpOp::Neq => Ok(x != y),
            _ => Err(ExprError::TypeMismatch("ordered comparison on bool".into())),
        },
        // Null is only equal to null. Cross-type Eq/Neq returns the obvious
        // identity answer (null != "x" is true) instead of failing — which is
        // what a policy author writing `input.recipient.address != null`
        // would expect. Ordered comparisons on null still fail (they make no
        // sense).
        (Value::Null, Value::Null) => match op {
            CmpOp::Eq => Ok(true),
            CmpOp::Neq => Ok(false),
            _ => Err(ExprError::TypeMismatch("ordered comparison on null".into())),
        },
        (Value::Null, _) | (_, Value::Null) => match op {
            CmpOp::Eq => Ok(false),
            CmpOp::Neq => Ok(true),
            _ => Err(ExprError::TypeMismatch(
                "ordered comparison with null".into(),
            )),
        },
        (a, b) => Err(ExprError::TypeMismatch(format!(
            "{op:?} between {a} and {b}"
        ))),
    }
}

fn truthy(v: &Value) -> Result<bool, ExprError> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(ExprError::NotBool),
    }
}

fn eval_node(node: &Node, ctx: &serde_json::Value) -> Result<Value, ExprError> {
    match node {
        Node::Bool(b) => Ok(Value::Bool(*b)),
        Node::Number(n) => Ok(Value::Number(*n)),
        Node::String(s) => Ok(Value::String(s.clone())),
        Node::Null => Ok(Value::Null),
        Node::Path(parts) => lookup(ctx, parts),
        Node::Cmp(l, op, r) => {
            let a = eval_node(l, ctx)?;
            let b = eval_node(r, ctx)?;
            Ok(Value::Bool(cmp(&a, *op, &b)?))
        }
        Node::And(l, r) => {
            let a = eval_node(l, ctx)?;
            if !truthy(&a)? {
                return Ok(Value::Bool(false));
            }
            let b = eval_node(r, ctx)?;
            Ok(Value::Bool(truthy(&b)?))
        }
        Node::Or(l, r) => {
            let a = eval_node(l, ctx)?;
            if truthy(&a)? {
                return Ok(Value::Bool(true));
            }
            let b = eval_node(r, ctx)?;
            Ok(Value::Bool(truthy(&b)?))
        }
        Node::Not(inner) => {
            let v = eval_node(inner, ctx)?;
            Ok(Value::Bool(!truthy(&v)?))
        }
    }
}

pub fn evaluate_bool(expr: &str, ctx: &serde_json::Value) -> Result<bool, ExprError> {
    let tokens = tokenize(expr)?;
    let mut parser = Parser { tokens, pos: 0 };
    let node = parser.parse_expr()?;
    // Reject trailing tokens. Without this, a malformed (or adversarial)
    // policy edit like `input.provider.trusted garbage_token` would silently
    // evaluate as `input.provider.trusted` and turn into an unintended allow.
    if !matches!(parser.peek(), Token::Eof) {
        return Err(ExprError::Parse(format!(
            "unexpected trailing tokens starting at {:?}",
            parser.peek()
        )));
    }
    let value = eval_node(&node, ctx)?;
    truthy(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> serde_json::Value {
        json!({
            "input": {
                "intent": "purchase_api_call",
                "payment_protocol": "x402",
                "amount_usd": 0.05,
                "emergency": { "freeze_all": false },
                "provider": { "trusted": true },
                "recipient": { "allowed": true }
            }
        })
    }

    #[test]
    fn evaluate_allow_rule() {
        let expr = "input.intent == \"purchase_api_call\" and input.payment_protocol == \"x402\" and input.amount_usd <= 0.50 and input.provider.trusted and input.recipient.allowed";
        assert!(evaluate_bool(expr, &ctx()).unwrap());
    }

    #[test]
    fn not_operator_works() {
        let mut c = ctx();
        c["input"]["recipient"]["allowed"] = json!(false);
        assert!(evaluate_bool("not input.recipient.allowed", &c).unwrap());
        assert!(!evaluate_bool("not input.provider.trusted", &c).unwrap());
    }

    #[test]
    fn equality_to_true() {
        let c = ctx();
        assert!(!evaluate_bool("input.emergency.freeze_all == true", &c).unwrap());
    }

    #[test]
    fn missing_path_errors() {
        let c = ctx();
        let err = evaluate_bool("input.nonexistent == true", &c).unwrap_err();
        assert!(matches!(err, ExprError::MissingPath(_)));
    }

    #[test]
    fn parens_and_or() {
        let c = ctx();
        assert!(evaluate_bool(
            "(input.provider.trusted) or (input.emergency.freeze_all)",
            &c
        )
        .unwrap());
        assert!(!evaluate_bool(
            "(not input.provider.trusted) and (not input.recipient.allowed)",
            &c
        )
        .unwrap());
    }

    #[test]
    fn trailing_tokens_are_rejected() {
        let c = ctx();
        let err = evaluate_bool("input.provider.trusted garbage_token", &c).expect_err("must fail");
        assert!(matches!(err, ExprError::Parse(_)), "got {err:?}");
        let err2 = evaluate_bool("true == true junk", &c).expect_err("must fail");
        assert!(matches!(err2, ExprError::Parse(_)), "got {err2:?}");
        // Sanity: well-formed expressions still evaluate.
        assert!(evaluate_bool("true == true", &c).unwrap());
    }

    #[test]
    fn null_comparisons_match_intuitive_semantics() {
        let c = json!({
            "input": {
                "addr_set":   "0x1111111111111111111111111111111111111111",
                "addr_unset": null
            }
        });
        // null == null is true; null != null is false (identity).
        assert!(evaluate_bool("input.addr_unset == null", &c).unwrap());
        assert!(!evaluate_bool("input.addr_unset != null", &c).unwrap());
        // string vs null returns the obvious identity answer the reviewer flagged
        // (`input.recipient.address != null` should be true when address is set).
        assert!(evaluate_bool("input.addr_set != null", &c).unwrap());
        assert!(!evaluate_bool("input.addr_set == null", &c).unwrap());
        // Ordered comparisons on null still error (no obvious meaning).
        // Both branches in cmp() must reject ordering: null-vs-null AND null-vs-T.
        let err_null_null =
            evaluate_bool("input.addr_unset < null", &c).expect_err("null < null must fail");
        assert!(
            matches!(err_null_null, ExprError::TypeMismatch(_)),
            "got {err_null_null:?}"
        );
        let err_string_null =
            evaluate_bool("input.addr_set < null", &c).expect_err("string < null must fail");
        assert!(
            matches!(err_string_null, ExprError::TypeMismatch(_)),
            "got {err_string_null:?}"
        );
        let err_null_string =
            evaluate_bool("null > input.addr_set", &c).expect_err("null > string must fail");
        assert!(
            matches!(err_null_string, ExprError::TypeMismatch(_)),
            "got {err_null_string:?}"
        );
    }
}
