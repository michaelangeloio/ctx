use std::collections::BTreeMap;

use crate::types::{PropType, Schema};

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("unknown node kind '{0}'")]
    UnknownKind(String),
    #[error("unknown property '{prop}' on {kind}")]
    UnknownProperty { kind: String, prop: String },
    #[error("missing required property '{prop}' on {kind}")]
    MissingRequired { kind: String, prop: String },
    #[error("invalid value '{value}' for enum property '{prop}' on {kind} (expected one of: {expected})")]
    InvalidEnum {
        kind: String,
        prop: String,
        value: String,
        expected: String,
    },
    #[error("invalid type for property '{prop}' on {kind}: expected {expected}, got '{value}'")]
    TypeMismatch {
        kind: String,
        prop: String,
        expected: String,
        value: String,
    },
    #[error("edge '{edge}' cannot connect {from_kind} -> {to_kind}")]
    InvalidEdgeEndpoints {
        edge: String,
        from_kind: String,
        to_kind: String,
    },
    #[error("unknown edge kind '{0}'")]
    UnknownEdge(String),
    #[error("empty value for required property '{prop}' on {kind}")]
    EmptyValue { kind: String, prop: String },
}

pub fn validate_node_props(
    schema: &Schema,
    kind: &str,
    props: &BTreeMap<String, serde_json::Value>,
    is_update: bool,
) -> Result<(), ValidationError> {
    let node_def = schema
        .nodes
        .get(kind)
        .ok_or_else(|| ValidationError::UnknownKind(kind.to_string()))?;

    for key in props.keys() {
        if !node_def.properties.contains_key(key) {
            return Err(ValidationError::UnknownProperty {
                kind: kind.to_string(),
                prop: key.clone(),
            });
        }
    }

    if !is_update {
        for (prop_name, prop_def) in &node_def.properties {
            if prop_def.required && !props.contains_key(prop_name) {
                return Err(ValidationError::MissingRequired {
                    kind: kind.to_string(),
                    prop: prop_name.clone(),
                });
            }
        }
    }

    for (key, value) in props {
        if value.is_null() {
            let prop_def = &node_def.properties[key];
            if prop_def.required {
                return Err(ValidationError::MissingRequired {
                    kind: kind.to_string(),
                    prop: key.clone(),
                });
            }
            continue;
        }
        let prop_def = &node_def.properties[key];
        validate_value(kind, key, &prop_def.prop_type, value, prop_def.required)?;
    }

    Ok(())
}

pub fn validate_edge(
    schema: &Schema,
    edge_kind: &str,
    from_kind: &str,
    to_kind: &str,
    props: &BTreeMap<String, serde_json::Value>,
) -> Result<(), ValidationError> {
    let edge_def = schema
        .edges
        .get(edge_kind)
        .ok_or_else(|| ValidationError::UnknownEdge(edge_kind.to_string()))?;

    if !edge_def.from.iter().any(|k| k == from_kind) || !edge_def.to.iter().any(|k| k == to_kind) {
        return Err(ValidationError::InvalidEdgeEndpoints {
            edge: edge_kind.to_string(),
            from_kind: from_kind.to_string(),
            to_kind: to_kind.to_string(),
        });
    }

    for key in props.keys() {
        if !edge_def.properties.contains_key(key) {
            return Err(ValidationError::UnknownProperty {
                kind: edge_kind.to_string(),
                prop: key.clone(),
            });
        }
    }

    for (key, value) in props {
        let prop_def = &edge_def.properties[key];
        validate_value(edge_kind, key, &prop_def.prop_type, value, prop_def.required)?;
    }

    Ok(())
}

fn validate_value(
    kind: &str,
    prop: &str,
    prop_type: &PropType,
    value: &serde_json::Value,
    required: bool,
) -> Result<(), ValidationError> {
    match prop_type {
        PropType::String | PropType::Timestamp => {
            match value.as_str() {
                None => return Err(ValidationError::TypeMismatch {
                    kind: kind.to_string(),
                    prop: prop.to_string(),
                    expected: "string".to_string(),
                    value: value.to_string(),
                }),
                Some(s) if required && s.trim().is_empty() => return Err(ValidationError::EmptyValue {
                    kind: kind.to_string(),
                    prop: prop.to_string(),
                }),
                _ => {}
            }
        }
        PropType::Int => {
            if !value.is_i64() && !value.is_u64() {
                return Err(ValidationError::TypeMismatch {
                    kind: kind.to_string(),
                    prop: prop.to_string(),
                    expected: "int".to_string(),
                    value: value.to_string(),
                });
            }
        }
        PropType::Float => {
            if !value.is_f64() && !value.is_i64() {
                return Err(ValidationError::TypeMismatch {
                    kind: kind.to_string(),
                    prop: prop.to_string(),
                    expected: "float".to_string(),
                    value: value.to_string(),
                });
            }
        }
        PropType::Bool => {
            if !value.is_boolean() {
                return Err(ValidationError::TypeMismatch {
                    kind: kind.to_string(),
                    prop: prop.to_string(),
                    expected: "bool".to_string(),
                    value: value.to_string(),
                });
            }
        }
        PropType::Enum(variants) => {
            let s = value.as_str().ok_or_else(|| ValidationError::TypeMismatch {
                kind: kind.to_string(),
                prop: prop.to_string(),
                expected: "string".to_string(),
                value: value.to_string(),
            })?;
            if !variants.iter().any(|v| v == s) {
                return Err(ValidationError::InvalidEnum {
                    kind: kind.to_string(),
                    prop: prop.to_string(),
                    value: s.to_string(),
                    expected: variants.join(", "),
                });
            }
        }
    }
    Ok(())
}

pub fn is_safe_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.as_bytes()[0].is_ascii_alphabetic()
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> Schema {
        crate::load_default_schema().unwrap()
    }

    #[test]
    fn valid_session_props() {
        let schema = test_schema();
        let mut props = BTreeMap::new();
        props.insert("session_id".into(), serde_json::json!("abc-123"));
        props.insert("title".into(), serde_json::json!("Fix bug"));
        props.insert("tool".into(), serde_json::json!("claude"));
        props.insert("model".into(), serde_json::json!("opus"));
        props.insert("project_path".into(), serde_json::json!("/app"));
        assert!(validate_node_props(&schema, "Session", &props, false).is_ok());
    }

    #[test]
    fn rejects_unknown_property() {
        let schema = test_schema();
        let mut props = BTreeMap::new();
        props.insert("session_id".into(), serde_json::json!("abc"));
        props.insert("title".into(), serde_json::json!("Fix"));
        props.insert("tool".into(), serde_json::json!("claude"));
        props.insert("model".into(), serde_json::json!("opus"));
        props.insert("project_path".into(), serde_json::json!("/app"));
        props.insert("mood".into(), serde_json::json!("happy"));
        let err = validate_node_props(&schema, "Session", &props, false).unwrap_err();
        assert!(matches!(err, ValidationError::UnknownProperty { .. }));
    }

    #[test]
    fn rejects_bad_enum_value() {
        let schema = test_schema();
        let mut props = BTreeMap::new();
        props.insert("session_id".into(), serde_json::json!("abc"));
        props.insert("title".into(), serde_json::json!("Fix"));
        props.insert("tool".into(), serde_json::json!("vim"));
        props.insert("model".into(), serde_json::json!("opus"));
        props.insert("project_path".into(), serde_json::json!("/app"));
        let err = validate_node_props(&schema, "Session", &props, false).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidEnum { .. }));
    }

    #[test]
    fn rejects_missing_required() {
        let schema = test_schema();
        let props = BTreeMap::new();
        let err = validate_node_props(&schema, "Session", &props, false).unwrap_err();
        assert!(matches!(err, ValidationError::MissingRequired { .. }));
    }

    #[test]
    fn allows_partial_on_update() {
        let schema = test_schema();
        let mut props = BTreeMap::new();
        props.insert("summary".into(), serde_json::json!("Done"));
        assert!(validate_node_props(&schema, "Session", &props, true).is_ok());
    }

    #[test]
    fn validates_edge_endpoints() {
        let schema = test_schema();
        let props = BTreeMap::new();
        assert!(validate_edge(&schema, "HAS_TOPIC", "Session", "Topic", &props).is_ok());
        let err = validate_edge(&schema, "HAS_TOPIC", "Session", "Project", &props).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidEdgeEndpoints { .. }));
    }
}
