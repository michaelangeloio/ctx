use ctx_db::sql::{self, Expr, Query, TableRef, NODE_COLS};
use ctx_db::{Database, DbError, Node};

use crate::edge_path::parse_edge_path;

pub fn walk(db: &Database, start_id: i64, edge_path: &str, limit: usize) -> Result<Vec<Node>, DbError> {
    let steps = parse_edge_path(edge_path);
    if steps.is_empty() {
        return Ok(vec![]);
    }

    for step in &steps {
        if let Some(k) = &step.kind {
            sql::require_ident(k)?;
            if !db.schema().edges.contains_key(k) {
                return Err(DbError::NotFound(format!("unknown edge kind '{k}'")));
            }
        }
    }

    let final_node = format!("n{}", steps.len());

    // Start with n0, accumulate JOINs through edge→node for each step
    let mut q = Query::from(TableRef::scan("node", "n0"))
        .distinct()
        .cols(&final_node, NODE_COLS);

    let mut prev = "n0".to_string();

    for (i, step) in steps.iter().enumerate() {
        let e = format!("e{i}");
        let n = format!("n{}", i + 1);

        let (src, tgt) = if step.reverse { ("to_id", "from_id") } else { ("from_id", "to_id") };

        // Edge join: prev.id = edge.{src} [AND edge.kind = ?]
        let mut on = Expr::eq(Expr::col(&prev, "id"), Expr::col(&e, src));
        if let Some(k) = &step.kind {
            on = Expr::and(on, Expr::eq(Expr::col(&e, "kind"), Expr::Str(k.clone())));
        }
        q = q.join(TableRef::scan("edge", &e), on);

        // Node join: edge.{tgt} = next.id
        q = q.join(
            TableRef::scan("node", &n),
            Expr::eq(Expr::col(&e, tgt), Expr::col(&n, "id")),
        );

        prev = n;
    }

    q = q
        .where_and(Expr::eq(Expr::col("n0", "id"), Expr::Int(start_id)))
        .order(&format!("{final_node}.id"), false)
        .limit(limit as i64);

    let compiled = q.build();
    let mut stmt = db.conn().prepare(compiled.sql())?;
    let rows = stmt.query_map(compiled.param_refs().as_slice(), Node::from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ctx_db::{CreateEdgeParams, CreateNodeParams};
    use std::collections::BTreeMap;

    fn db() -> Database {
        Database::open_in_memory(ctx_schema::load_default_schema().unwrap()).unwrap()
    }

    fn session(id: &str, title: &str) -> BTreeMap<String, serde_json::Value> {
        [
            ("session_id", serde_json::json!(id)),
            ("title", serde_json::json!(title)),
            ("tool", serde_json::json!("claude")),
            ("model", serde_json::json!("opus")),
            ("project_path", serde_json::json!("/app")),
        ].into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn single_hop() {
        let db = db();
        let s = db.create_node(CreateNodeParams { kind: "Session".into(), properties: session("s1", "S1") }).unwrap();
        let t = db.create_node(CreateNodeParams { kind: "Topic".into(), properties: [("name".into(), serde_json::json!("rust"))].into() }).unwrap();
        db.create_edge(CreateEdgeParams { kind: "HAS_TOPIC".into(), from_id: s.id, to_id: t.id, properties: BTreeMap::new() }).unwrap();

        let results = walk(&db, s.id, "HAS_TOPIC", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, t.id);
    }

    #[test]
    fn two_hops_reverse() {
        let db = db();
        let s1 = db.create_node(CreateNodeParams { kind: "Session".into(), properties: session("s1", "S1") }).unwrap();
        let s2 = db.create_node(CreateNodeParams { kind: "Session".into(), properties: session("s2", "S2") }).unwrap();
        let t = db.create_node(CreateNodeParams { kind: "Topic".into(), properties: [("name".into(), serde_json::json!("auth"))].into() }).unwrap();
        db.create_edge(CreateEdgeParams { kind: "HAS_TOPIC".into(), from_id: s1.id, to_id: t.id, properties: BTreeMap::new() }).unwrap();
        db.create_edge(CreateEdgeParams { kind: "HAS_TOPIC".into(), from_id: s2.id, to_id: t.id, properties: BTreeMap::new() }).unwrap();

        let results = walk(&db, s1.id, "HAS_TOPIC/~HAS_TOPIC", 10).unwrap();
        assert!(results.iter().any(|n| n.id == s2.id));
    }
}
