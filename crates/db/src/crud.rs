use std::collections::BTreeMap;

use duckdb::params;

use crate::sql::{self, Expr, Query, TableRef, NODE_COLS};
use crate::types::{Edge, Node};
use crate::{Database, DbError};
use ctx_schema::{validate_edge, validate_node_props};

pub struct CreateNodeParams {
    pub kind: String,
    pub properties: BTreeMap<String, serde_json::Value>,
}

pub struct CreateEdgeParams {
    pub kind: String,
    pub from_id: i64,
    pub to_id: i64,
    pub properties: BTreeMap<String, serde_json::Value>,
}

impl Database {
    pub fn create_node(&self, p: CreateNodeParams) -> Result<Node, DbError> {
        validate_node_props(&self.schema, &p.kind, &p.properties, false)?;
        let json = serde_json::to_string(&p.properties)?;
        self.conn.query_row(
            &format!("INSERT INTO node (kind, properties) VALUES (?, ?) RETURNING {}", sql::node_cols_csv()),
            params![p.kind, json],
            Node::from_row,
        ).map_err(DbError::from)
    }

    pub fn get_node_by_id(&self, id: i64) -> Result<Node, DbError> {
        self.conn
            .prepare(&format!("SELECT {} FROM node WHERE id = ?", sql::node_cols_csv()))?
            .query_row(params![id], Node::from_row)
            .map_err(|_| DbError::NotFound(format!("node {id}")))
    }

    pub fn resolve_ref(&self, kind: &str, id: i64) -> Result<Node, DbError> {
        self.conn
            .prepare(&format!("SELECT {} FROM node WHERE kind = ? AND id = ?", sql::node_cols_csv()))?
            .query_row(params![kind, id], Node::from_row)
            .map_err(|_| DbError::NotFound(format!("{kind}:{id}")))
    }

    pub fn update_node(&self, kind: &str, id: i64, props: BTreeMap<String, serde_json::Value>) -> Result<Node, DbError> {
        let existing = self.resolve_ref(kind, id)?;
        validate_node_props(&self.schema, &existing.kind, &props, true)?;

        let mut merged = existing.properties;
        for (key, value) in props {
            if value.is_null() {
                merged.remove(&key);
            } else {
                merged.insert(key, value);
            }
        }
        let json = serde_json::to_string(&merged)?;
        self.conn.execute("UPDATE node SET properties = ?, updated_at = now() WHERE id = ?", params![json, id])?;
        self.get_node_by_id(id)
    }

    pub fn change_node_kind(&self, kind: &str, id: i64, new_kind: &str) -> Result<Node, DbError> {
        let existing = self.resolve_ref(kind, id)?;

        if !self.schema.nodes.contains_key(new_kind) {
            return Err(DbError::Validation(ctx_schema::ValidationError::UnknownKind(new_kind.into())));
        }

        // Validate existing properties against the new kind's schema
        validate_node_props(&self.schema, new_kind, &existing.properties, false)?;

        self.conn.execute(
            "UPDATE node SET kind = ?, updated_at = now() WHERE id = ?",
            params![new_kind, id],
        )?;

        // Update from_kind/to_kind on edges referencing this node
        self.conn.execute("UPDATE edge SET from_kind = ? WHERE from_id = ?", params![new_kind, id])?;
        self.conn.execute("UPDATE edge SET to_kind = ? WHERE to_id = ?", params![new_kind, id])?;

        self.get_node_by_id(id)
    }

    pub fn delete_node(&self, kind: &str, id: i64) -> Result<(), DbError> {
        self.resolve_ref(kind, id)?;
        self.conn.execute("DELETE FROM edge WHERE from_id = ? OR to_id = ?", params![id, id])?;
        self.conn.execute("DELETE FROM node WHERE id = ?", params![id])?;
        Ok(())
    }

    pub fn list_nodes(&self, kind: &str, limit: usize) -> Result<Vec<Node>, DbError> {
        self.list_nodes_filtered(kind, limit, None, &[])
    }

    pub fn list_nodes_filtered(
        &self,
        kind: &str,
        limit: usize,
        order: Option<&str>,
        filters: &[Expr],
    ) -> Result<Vec<Node>, DbError> {
        if !self.schema.nodes.contains_key(kind) {
            return Err(DbError::Validation(ctx_schema::ValidationError::UnknownKind(kind.into())));
        }

        let (order_col, order_desc) = match order {
            Some(spec) => {
                let (col, dir) = spec.split_once(':').unwrap_or((spec, "desc"));
                let col = sql::require_ident(col)?;
                let desc = dir.eq_ignore_ascii_case("desc");
                (format!("n.properties->>'$.{col}'"), desc)
            }
            None => ("n.id".to_string(), true),
        };

        let mut q = Query::from(TableRef::scan("node", "n"))
            .cols("n", NODE_COLS)
            .where_and(Expr::eq(Expr::col("n", "kind"), Expr::Str(kind.into())));

        for filter in filters {
            q = q.where_and(filter.clone());
        }

        let q = q.order(&order_col, order_desc).limit(limit as i64).build();

        let mut stmt = self.conn.prepare(q.sql())?;
        let rows = stmt.query_map(q.param_refs().as_slice(), Node::from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn create_edge(&self, p: CreateEdgeParams) -> Result<Edge, DbError> {
        let from = self.get_node_by_id(p.from_id)?;
        let to = self.get_node_by_id(p.to_id)?;
        validate_edge(&self.schema, &p.kind, &from.kind, &to.kind, &p.properties)?;
        let json = serde_json::to_string(&p.properties)?;
        self.conn.query_row(
            &format!("INSERT INTO edge (kind, from_id, to_id, from_kind, to_kind, properties) VALUES (?, ?, ?, ?, ?, ?) RETURNING {}", sql::edge_cols_csv()),
            params![p.kind, p.from_id, p.to_id, from.kind, to.kind, json],
            Edge::from_row,
        ).map_err(DbError::from)
    }

    pub fn get_edge(&self, from_id: i64, kind: &str, to_id: i64) -> Result<Edge, DbError> {
        self.conn
            .prepare(&format!("SELECT {} FROM edge WHERE from_id = ? AND kind = ? AND to_id = ?", sql::edge_cols_csv()))?
            .query_row(params![from_id, kind, to_id], Edge::from_row)
            .map_err(|_| DbError::NotFound(format!("edge {from_id} -{kind}-> {to_id}")))
    }

    pub fn update_edge(
        &self,
        from_id: i64,
        kind: &str,
        to_id: i64,
        props: BTreeMap<String, serde_json::Value>,
    ) -> Result<Edge, DbError> {
        let existing = self.get_edge(from_id, kind, to_id)?;

        if let Some(edge_def) = self.schema.edges.get(kind) {
            for key in props.keys() {
                if !edge_def.properties.contains_key(key) {
                    return Err(DbError::Validation(ctx_schema::ValidationError::UnknownProperty {
                        kind: kind.into(),
                        prop: key.clone(),
                    }));
                }
            }
        }

        let mut merged = existing.properties;
        for (key, value) in props {
            if value.is_null() {
                merged.remove(&key);
            } else {
                merged.insert(key, value);
            }
        }
        let json = serde_json::to_string(&merged)?;
        self.conn.execute(
            "UPDATE edge SET properties = ? WHERE from_id = ? AND kind = ? AND to_id = ?",
            params![json, from_id, kind, to_id],
        )?;
        self.get_edge(from_id, kind, to_id)
    }

    pub fn delete_edge(&self, from_id: i64, kind: &str, to_id: i64) -> Result<(), DbError> {
        let affected = self.conn.execute(
            "DELETE FROM edge WHERE from_id = ? AND kind = ? AND to_id = ?",
            params![from_id, kind, to_id],
        )?;
        if affected == 0 { return Err(DbError::NotFound(format!("edge {from_id} -{kind}-> {to_id}"))); }
        Ok(())
    }

    pub fn list_edges(&self, node_id: i64) -> Result<Vec<Edge>, DbError> {
        let mut stmt = self.conn.prepare(
            &format!("SELECT {} FROM edge WHERE from_id = ? OR to_id = ? ORDER BY id", sql::edge_cols_csv()),
        )?;
        let rows = stmt.query_map(params![node_id, node_id], Edge::from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn node_count(&self) -> Result<i64, DbError> {
        Ok(self.conn.query_row("SELECT count(*) FROM node", [], |row| row.get(0))?)
    }

    pub fn edge_count(&self) -> Result<i64, DbError> {
        Ok(self.conn.query_row("SELECT count(*) FROM edge", [], |row| row.get(0))?)
    }

    pub fn kind_counts(&self) -> Result<Vec<(String, i64)>, DbError> {
        let mut stmt = self.conn.prepare("SELECT kind, count(*) FROM node GROUP BY kind ORDER BY count(*) DESC")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn count_by_kind(&self, kind: &str) -> Result<i64, DbError> {
        sql::require_ident(kind)?;
        let q = Query::from(TableRef::scan("node", "n"))
            .col_as(Expr::func("count", vec![Expr::Star]), "c")
            .where_and(Expr::eq(Expr::col("n", "kind"), Expr::Str(kind.into())))
            .build();
        Ok(self.conn.query_row(q.sql(), q.param_refs().as_slice(), |row| row.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let schema = ctx_schema::load_default_schema().unwrap();
        Database::open_in_memory(schema).unwrap()
    }

    fn session_props(id: &str) -> BTreeMap<String, serde_json::Value> {
        [
            ("session_id".into(), serde_json::json!(id)),
            ("title".into(), serde_json::json!("Test")),
            ("tool".into(), serde_json::json!("claude")),
            ("model".into(), serde_json::json!("opus")),
            ("project_path".into(), serde_json::json!("/app")),
        ].into()
    }

    #[test]
    fn create_and_get_node() {
        let db = test_db();
        let node = db.create_node(CreateNodeParams { kind: "Session".into(), properties: session_props("abc") }).unwrap();
        assert_eq!(node.kind, "Session");
        assert_eq!(db.get_node_by_id(node.id).unwrap().id, node.id);
    }

    #[test]
    fn update_node_merges() {
        let db = test_db();
        let node = db.create_node(CreateNodeParams { kind: "Session".into(), properties: session_props("abc") }).unwrap();
        let updated = db.update_node("Session", node.id, [("summary".into(), serde_json::json!("Done"))].into()).unwrap();
        assert_eq!(updated.properties["summary"], "Done");
        assert_eq!(updated.properties["title"], "Test");
    }

    #[test]
    fn delete_node_cascades_edges() {
        let db = test_db();
        let s = db.create_node(CreateNodeParams { kind: "Session".into(), properties: session_props("s1") }).unwrap();
        let t = db.create_node(CreateNodeParams { kind: "Topic".into(), properties: [("name".into(), serde_json::json!("rust"))].into() }).unwrap();
        db.create_edge(CreateEdgeParams { kind: "HAS_TOPIC".into(), from_id: s.id, to_id: t.id, properties: BTreeMap::new() }).unwrap();
        db.delete_node("Session", s.id).unwrap();
        assert!(db.list_edges(t.id).unwrap().is_empty());
    }

    #[test]
    fn rejects_invalid_edge_endpoints() {
        let db = test_db();
        let s = db.create_node(CreateNodeParams { kind: "Session".into(), properties: session_props("s1") }).unwrap();
        let p = db.create_node(CreateNodeParams { kind: "Project".into(), properties: [("name".into(), serde_json::json!("x")), ("path".into(), serde_json::json!("/x"))].into() }).unwrap();
        let err = db.create_edge(CreateEdgeParams { kind: "HAS_TOPIC".into(), from_id: s.id, to_id: p.id, properties: BTreeMap::new() }).unwrap_err();
        assert!(err.to_string().contains("cannot connect"));
    }
}
