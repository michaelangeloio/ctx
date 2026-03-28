use ctx_db::sql::{self, Cte, Expr, Query, TableRef};
use ctx_db::{Database, DbError};

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: i64,
    pub kind: String,
    pub label: String,
    pub score: f64,
}

pub fn rebuild_fts_index(db: &Database) -> Result<usize, DbError> {
    let conn = db.conn();
    conn.execute_batch("INSTALL fts; LOAD fts;")?;
    let _ = conn.execute_batch("PRAGMA drop_fts_index('node')");
    conn.execute_batch(
        "PRAGMA create_fts_index('node', 'id', 'properties', stemmer = 'porter', stopwords = 'english')",
    )?;
    let count: i64 = conn.query_row("SELECT count(*) FROM node", [], |row| row.get(0))?;
    Ok(count as usize)
}

pub fn search(
    db: &Database,
    query_text: &str,
    kind_filter: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchResult>, DbError> {
    if let Some(k) = kind_filter { sql::require_ident(k)?; }

    let mut fuzzy = Query::from(TableRef::scan("node", "n"))
        .col("n", "id")
        .col("n", "kind")
        .col("n", "properties")
        .col_as(
            Expr::func("greatest", vec![
                Expr::func("jaro_winkler_similarity", vec![
                    Expr::bare("n.properties->>'$.title'"), Expr::Str(query_text.into()),
                ]),
                Expr::func("jaro_winkler_similarity", vec![
                    Expr::bare("n.properties->>'$.name'"), Expr::Str(query_text.into()),
                ]),
                Expr::func("jaro_winkler_similarity", vec![
                    Expr::bare("n.properties->>'$.summary'"), Expr::Str(query_text.into()),
                ]),
            ]),
            "score",
        );
    if let Some(k) = kind_filter {
        fuzzy = fuzzy.where_and(Expr::eq(Expr::col("n", "kind"), Expr::Str(k.into())));
    }

    let mut exact = Query::from(TableRef::scan("node", "x"))
        .col("x", "id")
        .where_and(Expr::ilike(Expr::col("x", "properties"), Expr::Str(format!("%{query_text}%"))));
    if let Some(k) = kind_filter {
        exact = exact.where_and(Expr::eq(Expr::col("x", "kind"), Expr::Str(k.into())));
    }

    // Window functions and correlated subqueries are expressed as Bare since the
    // AST doesn't model them. All user values are still parameterized above.
    let ranked = Query::from(TableRef::cte("fuzzy", "f"))
        .col("f", "id").col("f", "kind").col("f", "properties")
        .col_as(
            Expr::bare(
                "(1.0 / (60.0 + ROW_NUMBER() OVER (ORDER BY f.score DESC))) \
                 + CASE WHEN f.id IN (SELECT e2.id FROM exact AS e2) THEN 0.05 ELSE 0.0 END"
            ),
            "rrf",
        )
        .where_and(Expr::bare("f.score > 0.6"));

    let outer = Query::from(TableRef::cte("ranked", "r"))
        .col("r", "id").col("r", "kind")
        .col_as(
            Expr::func("coalesce", vec![
                Expr::bare("r.properties->>'$.title'"),
                Expr::bare("r.properties->>'$.name'"),
                Expr::Str("".into()),
            ]),
            "label",
        )
        .col_bare("rrf")
        .order("rrf", true)
        .limit(limit as i64)
        .with_cte(Cte { name: "fuzzy".into(), recursive: false, base: fuzzy, recursive_term: None })
        .with_cte(Cte { name: "exact".into(), recursive: false, base: exact, recursive_term: None })
        .with_cte(Cte { name: "ranked".into(), recursive: false, base: ranked, recursive_term: None });

    let compiled = outer.build();
    let mut stmt = db.conn().prepare(compiled.sql())?;
    let rows = stmt.query_map(compiled.param_refs().as_slice(), |row| {
        Ok(SearchResult { id: row.get(0)?, kind: row.get(1)?, label: row.get(2)?, score: row.get(3)? })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}
