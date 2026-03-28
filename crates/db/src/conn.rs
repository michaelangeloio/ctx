use std::path::{Path, PathBuf};

use ctx_schema::Schema;
use duckdb::Connection;

use crate::schema_registry;
use crate::DbError;

const SCHEMA_DDL: &str = "
CREATE SEQUENCE IF NOT EXISTS node_id_seq;
CREATE SEQUENCE IF NOT EXISTS edge_id_seq;

CREATE TABLE IF NOT EXISTS node (
    id          INTEGER PRIMARY KEY DEFAULT nextval('node_id_seq'),
    kind        VARCHAR NOT NULL,
    properties  JSON    NOT NULL DEFAULT '{}',
    created_at  TIMESTAMP NOT NULL DEFAULT now(),
    updated_at  TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS edge (
    id         INTEGER PRIMARY KEY DEFAULT nextval('edge_id_seq'),
    kind       VARCHAR NOT NULL,
    from_id    INTEGER NOT NULL,
    to_id      INTEGER NOT NULL,
    from_kind  VARCHAR NOT NULL,
    to_kind    VARCHAR NOT NULL,
    properties JSON    NOT NULL DEFAULT '{}',
    created_at TIMESTAMP NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_node_kind ON node(kind);
CREATE INDEX IF NOT EXISTS idx_edge_from ON edge(from_id, kind);
CREATE INDEX IF NOT EXISTS idx_edge_to   ON edge(to_id, kind);
CREATE INDEX IF NOT EXISTS idx_edge_kind ON edge(kind);
";

pub struct Database {
    pub(crate) conn: Connection,
    pub(crate) schema: Schema,
}

impl Database {
    pub fn open_default(schema: Schema) -> Result<Self, DbError> {
        let path = default_db_path()?;
        Self::open(&path, schema)
    }

    pub fn open(path: &Path, schema: Schema) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Self { conn, schema };
        db.ensure_schema()?;
        Ok(db)
    }

    pub fn open_in_memory(schema: Schema) -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn, schema };
        db.ensure_schema()?;
        Ok(db)
    }

    fn ensure_schema(&self) -> Result<(), DbError> {
        self.conn.execute_batch(SCHEMA_DDL)?;
        schema_registry::sync_views(&self.conn, &self.schema)?;
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }
}

fn default_db_path() -> Result<PathBuf, DbError> {
    let base = if let Ok(path) = std::env::var("CTX_DB") {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .ok_or_else(|| DbError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not determine home directory",
            )))?
            .join(".ctx")
            .join("ctx.db")
    };
    Ok(base)
}
