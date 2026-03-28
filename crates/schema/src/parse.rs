use std::collections::BTreeMap;
use std::path::Path;

use crate::types::{EdgeDef, NodeDef, PropDef, PropType, Schema};
use crate::SchemaError;

pub fn load_schema(path: &Path) -> Result<Schema, SchemaError> {
    let content = std::fs::read_to_string(path)?;
    parse_schema(&content)
}

pub fn parse_schema(content: &str) -> Result<Schema, SchemaError> {
    let raw: toml::Value = toml::from_str(content)
        .map_err(|e| SchemaError::Parse(e.to_string()))?;

    let nodes = parse_nodes(raw.get("nodes"))?;
    let edges = parse_edges(raw.get("edges"), &nodes)?;

    Ok(Schema { nodes, edges })
}

fn parse_nodes(val: Option<&toml::Value>) -> Result<BTreeMap<String, NodeDef>, SchemaError> {
    let table = match val {
        Some(toml::Value::Table(t)) => t,
        Some(_) => return Err(SchemaError::Parse("'nodes' must be a table".into())),
        None => return Ok(BTreeMap::new()),
    };

    let mut nodes = BTreeMap::new();
    for (name, props_val) in table {
        let props_table = props_val
            .as_table()
            .ok_or_else(|| SchemaError::Parse(format!("node '{name}' must be a table")))?;

        let mut properties = BTreeMap::new();
        for (prop_name, type_val) in props_table {
            properties.insert(prop_name.clone(), parse_prop_value(type_val, &format!("{name}.{prop_name}"))?);
        }

        nodes.insert(
            name.clone(),
            NodeDef {
                name: name.clone(),
                properties,
            },
        );
    }

    Ok(nodes)
}

fn parse_edges(
    val: Option<&toml::Value>,
    nodes: &BTreeMap<String, NodeDef>,
) -> Result<BTreeMap<String, EdgeDef>, SchemaError> {
    let table = match val {
        Some(toml::Value::Table(t)) => t,
        Some(_) => return Err(SchemaError::Parse("'edges' must be a table".into())),
        None => return Ok(BTreeMap::new()),
    };

    let mut edges = BTreeMap::new();
    for (name, edge_val) in table {
        let edge_table = edge_val
            .as_table()
            .ok_or_else(|| SchemaError::Parse(format!("edge '{name}' must be a table")))?;

        let from = parse_string_array(edge_table.get("from"), name, "from")?;
        let to = parse_string_array(edge_table.get("to"), name, "to")?;

        for kind in from.iter().chain(to.iter()) {
            if !nodes.contains_key(kind) {
                return Err(SchemaError::NodeNotFound(format!(
                    "edge '{name}' references unknown node kind '{kind}'"
                )));
            }
        }

        let mut properties = BTreeMap::new();
        for (key, val) in edge_table {
            if key == "from" || key == "to" {
                continue;
            }
            // Edge properties can be strings or tables (same as node properties)
            // but skip array values (those are from/to which we already handled)
            if val.is_array() { continue; }
            properties.insert(key.clone(), parse_prop_value(val, &format!("{name}.{key}"))?);
        }

        edges.insert(
            name.clone(),
            EdgeDef {
                name: name.clone(),
                from,
                to,
                properties,
            },
        );
    }

    Ok(edges)
}

fn parse_string_array(
    val: Option<&toml::Value>,
    edge: &str,
    field: &str,
) -> Result<Vec<String>, SchemaError> {
    let arr = val
        .and_then(|v| v.as_array())
        .ok_or_else(|| SchemaError::Parse(format!("edge '{edge}.{field}' must be an array")))?;

    arr.iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| SchemaError::Parse(format!("edge '{edge}.{field}' values must be strings")))
        })
        .collect()
}

/// Parse a property value — supports both formats:
///   name = "string"
///   name = { type = "string", hint = "how to fill this in" }
fn parse_prop_value(val: &toml::Value, context: &str) -> Result<PropDef, SchemaError> {
    match val {
        toml::Value::String(s) => parse_type_str(s, None),
        toml::Value::Table(t) => {
            let type_str = t.get("type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SchemaError::Parse(format!("{context}: table property must have a 'type' field")))?;
            let hint = t.get("hint").and_then(|v| v.as_str()).map(|s| s.to_string());
            parse_type_str(type_str, hint)
        }
        _ => Err(SchemaError::Parse(format!("{context}: must be a string or table"))),
    }
}

fn parse_type_str(type_str: &str, hint: Option<String>) -> Result<PropDef, SchemaError> {
    let (base, required) = if let Some(stripped) = type_str.strip_suffix('?') {
        (stripped, false)
    } else {
        (type_str, true)
    };

    let prop_type = if let Some(variants) = base.strip_prefix("enum:") {
        let values: Vec<String> = variants.split(',').map(|s| s.trim().to_string()).collect();
        if values.is_empty() {
            return Err(SchemaError::InvalidType(type_str.to_string()));
        }
        PropType::Enum(values)
    } else {
        match base {
            "string" => PropType::String,
            "int" => PropType::Int,
            "float" => PropType::Float,
            "bool" => PropType::Bool,
            "timestamp" => PropType::Timestamp,
            _ => return Err(SchemaError::InvalidType(type_str.to_string())),
        }
    };

    Ok(PropDef { prop_type, required, hint })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_schema() {
        let schema = crate::load_default_schema().unwrap();
        assert!(schema.nodes.contains_key("Session"));
        assert!(schema.nodes.contains_key("Topic"));
        assert!(schema.edges.contains_key("HAS_TOPIC"));
        assert!(schema.edges.contains_key("CONTINUES"));
    }

    #[test]
    fn parse_required_and_optional() {
        let schema = crate::load_default_schema().unwrap();
        let session = &schema.nodes["Session"];
        assert!(session.properties["title"].required);
        assert!(!session.properties["summary"].required);
    }

    #[test]
    fn parse_enum_type() {
        let schema = crate::load_default_schema().unwrap();
        let tool = &schema.nodes["Session"].properties["tool"];
        assert!(matches!(&tool.prop_type, PropType::Enum(v) if v == &["claude", "codex"]));
    }

    #[test]
    fn edge_validates_node_kinds() {
        let result = parse_schema(
            r#"
            [nodes.Foo]
            name = "string"

            [edges.BAD]
            from = ["Foo"]
            to = ["NonExistent"]
            "#,
        );
        assert!(result.is_err());
    }
}
