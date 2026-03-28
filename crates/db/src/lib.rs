mod conn;
mod crud;
mod retry;
mod schema_registry;
pub mod sql;
mod types;

pub use conn::Database;
pub use crud::{CreateEdgeParams, CreateNodeParams};
pub use retry::RetryConfig;
pub use types::{Edge, Node};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("duckdb error: {0}")]
    DuckDb(#[from] duckdb::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("schema error: {0}")]
    Schema(#[from] ctx_schema::SchemaError),
    #[error("validation error: {0}")]
    Validation(#[from] ctx_schema::ValidationError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database locked after {0} retries")]
    Locked(u32),
}
