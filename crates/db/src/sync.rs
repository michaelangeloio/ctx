use crate::{Database, DbError};

pub struct SyncStats {
    pub nodes_exported: usize,
    pub edges_exported: usize,
}

pub struct MergeStats {
    pub nodes_added: usize,
    pub nodes_updated: usize,
    pub edges_added: usize,
}

pub fn setup_gcs(db: &Database) -> Result<(), DbError> {
    db.conn.execute_batch(
        "INSTALL httpfs; LOAD httpfs;
         SET force_download = true;"
    )?;

    // GCS via S3 compat -- credentials from env vars CTX_GCS_KEY_ID / CTX_GCS_SECRET
    let key_id = std::env::var("CTX_GCS_KEY_ID")
        .map_err(|_| DbError::NotFound("CTX_GCS_KEY_ID env var not set".into()))?;
    let secret = std::env::var("CTX_GCS_SECRET")
        .map_err(|_| DbError::NotFound("CTX_GCS_SECRET env var not set".into()))?;

    db.conn.execute_batch(&format!(
        "CREATE OR REPLACE SECRET ctx_gcs (
            TYPE GCS,
            KEY_ID '{key_id}',
            SECRET '{secret}'
        );"
    ))?;
    Ok(())
}

pub fn export_to_gcs(db: &Database, bucket: &str, namespace: &str) -> Result<SyncStats, DbError> {
    setup_gcs(db)?;

    let prefix = format!("gs://{bucket}/ctx/{namespace}");

    let node_count: i64 = db.conn
        .query_row("SELECT count(*) FROM node", [], |row| row.get(0))?;
    let edge_count: i64 = db.conn
        .query_row("SELECT count(*) FROM edge", [], |row| row.get(0))?;

    db.conn.execute_batch(&format!(
        "COPY node TO '{prefix}/nodes.parquet' (FORMAT PARQUET, OVERWRITE_OR_IGNORE);
         COPY edge TO '{prefix}/edges.parquet' (FORMAT PARQUET, OVERWRITE_OR_IGNORE);"
    ))?;

    Ok(SyncStats {
        nodes_exported: node_count as usize,
        edges_exported: edge_count as usize,
    })
}

pub fn import_and_merge(db: &Database, bucket: &str, namespace: &str) -> Result<MergeStats, DbError> {
    setup_gcs(db)?;

    let prefix = format!("gs://{bucket}/ctx/{namespace}");

    // Load remote data into temp tables
    db.conn.execute_batch(&format!(
        "CREATE OR REPLACE TEMP TABLE remote_node AS
             SELECT * FROM '{prefix}/nodes.parquet';
         CREATE OR REPLACE TEMP TABLE remote_edge AS
             SELECT * FROM '{prefix}/edges.parquet';"
    ))?;

    // Merge nodes: insert those whose (kind, properties) don't match any local node
    // Use natural keys per kind to detect duplicates
    // Natural key extraction: session_id for Session, name for Topic/Project/Branch/Model,
    // content for Highlight, name+url for Artifact
    let nodes_before: i64 = db.conn
        .query_row("SELECT count(*) FROM node", [], |row| row.get(0))?;

    db.conn.execute_batch(
        "-- Insert remote nodes that don't exist locally (by kind + natural key)
         INSERT INTO node (kind, properties, created_at, updated_at)
         SELECT rn.kind, rn.properties, rn.created_at, rn.updated_at
         FROM remote_node rn
         WHERE NOT EXISTS (
             SELECT 1 FROM node ln
             WHERE ln.kind = rn.kind
             AND (
                 -- Session: match by session_id
                 (rn.kind = 'Session' AND json_extract_string(ln.properties, '$.session_id') = json_extract_string(rn.properties, '$.session_id'))
                 -- Name-based kinds
                 OR (rn.kind IN ('Topic','Project','Branch','Model')
                     AND json_extract_string(ln.properties, '$.name') = json_extract_string(rn.properties, '$.name'))
                 -- Highlight: match by content
                 OR (rn.kind = 'Highlight' AND json_extract_string(ln.properties, '$.content') = json_extract_string(rn.properties, '$.content'))
                 -- Artifact: match by name+url
                 OR (rn.kind = 'Artifact'
                     AND json_extract_string(ln.properties, '$.name') = json_extract_string(rn.properties, '$.name')
                     AND COALESCE(json_extract_string(ln.properties, '$.url'),'') = COALESCE(json_extract_string(rn.properties, '$.url'),''))
             )
         );"
    )?;

    let nodes_after: i64 = db.conn
        .query_row("SELECT count(*) FROM node", [], |row| row.get(0))?;
    let nodes_added = (nodes_after - nodes_before) as usize;

    // Build ID mapping: remote_id -> local_id for nodes that exist in both
    // For newly inserted nodes, they got new local IDs
    // Build ID mapping: prefer exact ID match, then natural key match
    db.conn.execute_batch(
        "CREATE OR REPLACE TEMP TABLE id_map AS
         WITH exact AS (
             SELECT rn.id AS remote_id, ln.id AS local_id, 1 AS priority
             FROM remote_node rn
             JOIN node ln ON ln.id = rn.id AND ln.kind = rn.kind
         ),
         nk_match AS (
             SELECT rn.id AS remote_id, FIRST(ln.id) AS local_id, 2 AS priority
             FROM remote_node rn
             JOIN node ln ON ln.kind = rn.kind
             AND (
                 (rn.kind = 'Session' AND json_extract_string(ln.properties, '$.session_id') = json_extract_string(rn.properties, '$.session_id'))
                 OR (rn.kind IN ('Topic','Project','Branch','Model')
                     AND json_extract_string(ln.properties, '$.name') = json_extract_string(rn.properties, '$.name'))
                 OR (rn.kind = 'Highlight' AND json_extract_string(ln.properties, '$.content') = json_extract_string(rn.properties, '$.content'))
                 OR (rn.kind = 'Artifact'
                     AND json_extract_string(ln.properties, '$.name') = json_extract_string(rn.properties, '$.name')
                     AND COALESCE(json_extract_string(ln.properties, '$.url'),'') = COALESCE(json_extract_string(rn.properties, '$.url'),''))
             )
             WHERE rn.id NOT IN (SELECT remote_id FROM exact)
             GROUP BY rn.id
         ),
         combined AS (
             SELECT * FROM exact
             UNION ALL
             SELECT * FROM nk_match
         )
         SELECT remote_id, local_id FROM combined;"
    )?;

    // Merge edges: remap IDs and insert missing ones
    let edges_before: i64 = db.conn
        .query_row("SELECT count(*) FROM edge", [], |row| row.get(0))?;

    db.conn.execute_batch(
        "INSERT INTO edge (kind, from_id, to_id, from_kind, to_kind, properties, created_at)
         SELECT re.kind, fm.local_id, tm.local_id, re.from_kind, re.to_kind, re.properties, re.created_at
         FROM remote_edge re
         JOIN id_map fm ON fm.remote_id = re.from_id
         JOIN id_map tm ON tm.remote_id = re.to_id
         WHERE NOT EXISTS (
             SELECT 1 FROM edge le
             WHERE le.from_id = fm.local_id
             AND le.kind = re.kind
             AND le.to_id = tm.local_id
         );"
    )?;

    let edges_after: i64 = db.conn
        .query_row("SELECT count(*) FROM edge", [], |row| row.get(0))?;

    // Cleanup
    db.conn.execute_batch(
        "DROP TABLE IF EXISTS remote_node;
         DROP TABLE IF EXISTS remote_edge;
         DROP TABLE IF EXISTS id_map;"
    )?;

    Ok(MergeStats {
        nodes_added,
        nodes_updated: 0,
        edges_added: (edges_after - edges_before) as usize,
    })
}
