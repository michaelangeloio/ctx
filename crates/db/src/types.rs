use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Node {
    pub id: i64,
    pub kind: String,
    pub properties: BTreeMap<String, serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

impl Node {
    pub fn label(&self) -> &str {
        self.properties
            .get("title")
            .or_else(|| self.properties.get("name"))
            .or_else(|| self.properties.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    pub fn ref_str(&self) -> String {
        format!("{}:{}", self.kind, self.id)
    }

    /// Parse a Node from a DuckDB row.
    /// Expects columns: id, kind, properties, created_at::VARCHAR, updated_at::VARCHAR
    pub fn from_row(row: &duckdb::Row<'_>) -> duckdb::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            kind: row.get(1)?,
            properties: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(),
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Edge {
    pub id: i64,
    pub kind: String,
    pub from_id: i64,
    pub to_id: i64,
    pub from_kind: String,
    pub to_kind: String,
    pub properties: BTreeMap<String, serde_json::Value>,
    pub created_at: String,
}

impl Edge {
    /// Parse an Edge from a DuckDB row.
    /// Expects columns: id, kind, from_id, to_id, from_kind, to_kind, properties, created_at::VARCHAR
    pub fn from_row(row: &duckdb::Row<'_>) -> duckdb::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            kind: row.get(1)?,
            from_id: row.get(2)?,
            to_id: row.get(3)?,
            from_kind: row.get(4)?,
            to_kind: row.get(5)?,
            properties: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
            created_at: row.get(7)?,
        })
    }
}
