mod parse;
mod types;
mod validate;
pub mod views;

pub use parse::load_schema;
pub use types::{EdgeDef, NodeDef, PropDef, PropType, Schema};
pub use validate::{ValidationError, validate_node_props, validate_edge, is_safe_identifier};
pub use views::generate_view_ddl;

const DEFAULT_SCHEMA: &str = include_str!("../../../config/schema.toml");

pub fn load_default_schema() -> Result<Schema, SchemaError> {
    parse::parse_schema(DEFAULT_SCHEMA)
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("failed to parse schema: {0}")]
    Parse(String),
    #[error("invalid property type '{0}'")]
    InvalidType(String),
    #[error("node kind '{0}' not found")]
    NodeNotFound(String),
    #[error("edge kind '{0}' not found")]
    EdgeNotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
