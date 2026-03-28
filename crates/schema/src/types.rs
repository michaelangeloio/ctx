use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Schema {
    pub nodes: BTreeMap<String, NodeDef>,
    pub edges: BTreeMap<String, EdgeDef>,
}

#[derive(Debug, Clone)]
pub struct NodeDef {
    pub name: String,
    pub properties: BTreeMap<String, PropDef>,
}

impl NodeDef {
    pub fn required_props(&self) -> impl Iterator<Item = (&str, &PropDef)> {
        self.properties
            .iter()
            .filter(|(_, p)| p.required)
            .map(|(k, v)| (k.as_str(), v))
    }

    pub fn optional_props(&self) -> impl Iterator<Item = (&str, &PropDef)> {
        self.properties
            .iter()
            .filter(|(_, p)| !p.required)
            .map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Debug, Clone)]
pub struct EdgeDef {
    pub name: String,
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub properties: BTreeMap<String, PropDef>,
}

#[derive(Debug, Clone)]
pub struct PropDef {
    pub prop_type: PropType,
    pub required: bool,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropType {
    String,
    Int,
    Float,
    Bool,
    Timestamp,
    Enum(Vec<String>),
}

impl PropType {
    pub fn duckdb_type(&self) -> &'static str {
        match self {
            PropType::String | PropType::Enum(_) => "VARCHAR",
            PropType::Int => "INTEGER",
            PropType::Float => "DOUBLE",
            PropType::Bool => "BOOLEAN",
            PropType::Timestamp => "TIMESTAMP",
        }
    }
}
