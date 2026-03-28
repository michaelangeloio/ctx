use ctx_schema::{views::generate_view_ddl, Schema};
use duckdb::Connection;

use crate::DbError;

pub fn sync_views(conn: &Connection, schema: &Schema) -> Result<(), DbError> {
    let ddls = generate_view_ddl(schema);
    for ddl in ddls {
        conn.execute_batch(&ddl)?;
    }
    Ok(())
}
