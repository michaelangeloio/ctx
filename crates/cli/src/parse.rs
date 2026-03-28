use std::collections::BTreeMap;

use ctx_db::DbError;
use ctx_db::sql::{Expr, Op};

pub fn node_ref(s: &str) -> Result<(String, i64), DbError> {
    let (kind, id_str) = s
        .split_once(':')
        .ok_or_else(|| DbError::NotFound(format!("invalid ref '{s}', expected Kind:id")))?;

    let id: i64 = id_str
        .parse()
        .map_err(|_| DbError::NotFound(format!("invalid id '{id_str}' in ref '{s}'")))?;

    Ok((kind.to_string(), id))
}

pub fn key_value_pairs(args: &[String]) -> BTreeMap<String, serde_json::Value> {
    let mut map = BTreeMap::new();
    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            let json_value = parse_value(value);
            map.insert(key.to_string(), json_value);
        }
    }
    map
}

fn parse_value(s: &str) -> serde_json::Value {
    if s == "null" {
        return serde_json::Value::Null;
    }
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::Value::Number(n.into());
    }
    if let Ok(f) = s.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return serde_json::Value::Number(n);
        }
    }
    if s == "true" {
        return serde_json::Value::Bool(true);
    }
    if s == "false" {
        return serde_json::Value::Bool(false);
    }
    serde_json::Value::String(s.to_string())
}

/// Parse filter arguments like `key=value`, `key>value`, `key~value` into Expr conditions.
/// Multiple filters in a single arg are comma-separated: `tool=claude,kind=discovery`
pub fn parse_filters(args: &[String]) -> Result<Vec<Expr>, DbError> {
    let mut exprs = Vec::new();
    for arg in args {
        // skip anything that looks like a flag (--limit, etc.)
        if arg.starts_with('-') { continue; }
        for part in arg.split(',') {
            if let Some(expr) = parse_single_filter(part)? {
                exprs.push(expr);
            }
        }
    }
    Ok(exprs)
}

fn parse_single_filter(s: &str) -> Result<Option<Expr>, DbError> {
    // order matters: check multi-char operators before single-char
    let operators: &[(&str, Op)] = &[
        ("!=", Op::Ne),
        (">=", Op::Ge),
        ("<=", Op::Le),
        (">", Op::Gt),
        ("<", Op::Lt),
        ("~", Op::ILike),
        ("^", Op::ILike), // starts_with — handled specially below
        ("=", Op::Eq),
    ];

    for &(token, op) in operators {
        if let Some((key, value)) = s.split_once(token) {
            if key.is_empty() { continue; }
            ctx_db::sql::require_ident(key)?;

            let col = Expr::bare(&format!("n.properties->>'$.{key}'"));

            let expr = match token {
                "~" => Expr::ilike(col, Expr::Str(format!("%{value}%"))),
                "^" => Expr::ilike(col, Expr::Str(format!("{value}%"))),
                _ => {
                    let val = parse_value(value);
                    let rhs = match val {
                        serde_json::Value::Number(n) => Expr::Int(n.as_i64().unwrap_or(0)),
                        serde_json::Value::String(s) => Expr::Str(s),
                        serde_json::Value::Bool(b) => Expr::Str(b.to_string()),
                        _ => Expr::Str(value.to_string()),
                    };
                    Expr::BinOp { op, left: col.into(), right: rhs.into() }
                }
            };
            return Ok(Some(expr));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ref() {
        let (kind, id) = node_ref("Session:42").unwrap();
        assert_eq!(kind, "Session");
        assert_eq!(id, 42);
    }

    #[test]
    fn parse_kv() {
        let args = vec![
            "title=Fix bug".to_string(),
            "count=42".to_string(),
            "draft=true".to_string(),
        ];
        let map = key_value_pairs(&args);
        assert_eq!(map["title"], serde_json::json!("Fix bug"));
        assert_eq!(map["count"], serde_json::json!(42));
        assert_eq!(map["draft"], serde_json::json!(true));
    }

    #[test]
    fn parse_eq_filter() {
        let filters = parse_filters(&["kind=blocker".into()]).unwrap();
        assert_eq!(filters.len(), 1);
    }

    #[test]
    fn parse_comma_separated_filters() {
        let filters = parse_filters(&["tool=claude,model=opus".into()]).unwrap();
        assert_eq!(filters.len(), 2);
    }

    #[test]
    fn parse_contains_filter() {
        let filters = parse_filters(&["title~JWT".into()]).unwrap();
        assert_eq!(filters.len(), 1);
    }

    #[test]
    fn rejects_bad_identifier() {
        assert!(parse_filters(&["bad key=value".into()]).is_err());
    }

    #[test]
    fn skips_flags() {
        let filters = parse_filters(&["--limit".into(), "10".into(), "kind=blocker".into()]).unwrap();
        assert_eq!(filters.len(), 1);
    }
}
