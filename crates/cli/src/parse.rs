use std::collections::BTreeMap;

use ctx_db::DbError;

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
}
