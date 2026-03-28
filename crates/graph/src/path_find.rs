use ctx_db::sql::{self, Cte, Expr, Query, TableRef};
use ctx_db::{Database, DbError};

#[derive(Debug, Clone)]
pub struct PathResult {
    pub hops: Vec<PathHop>,
}

#[derive(Debug, Clone)]
pub struct PathHop {
    pub node_id: i64,
    pub node_kind: String,
    pub node_label: String,
    pub edge_kind: Option<String>,
    pub direction: Option<Direction>,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Outgoing,
    Incoming,
}

pub fn shortest_path(
    db: &Database,
    from_id: i64,
    to_id: i64,
    max_depth: usize,
    via_kinds: Option<&[&str]>,
) -> Result<Option<PathResult>, DbError> {
    let max_depth = max_depth.min(5);

    if let Some(kinds) = via_kinds {
        for k in kinds { sql::require_ident(k)?; }
    }

    // -- Base case: seed the traversal from `from_id` --
    let base = Query::from(TableRef::scan("node", "n"))
        .col_as(Expr::Int(from_id), "start_id")
        .col_as(Expr::col("n", "id"), "cur")
        .col_as(Expr::func("list_value", vec![Expr::col("n", "id")]), "visited")
        .col_as(
            Expr::cast(Expr::func("list_value", vec![]), "VARCHAR[]"),
            "trail",
        )
        .col_as(Expr::Int(0), "depth")
        .where_and(Expr::eq(Expr::col("n", "id"), Expr::Int(from_id)));

    let recursive = Query::from(TableRef::cte("paths", "p"))
        .join(TableRef::scan("edge", "e"), {
            let mut cond = Expr::or(
                Expr::eq(Expr::col("e", "from_id"), Expr::bare("p.cur")),
                Expr::eq(Expr::col("e", "to_id"), Expr::bare("p.cur")),
            );
            if let Some(kinds) = via_kinds {
                let kind_exprs: Vec<Expr> = kinds.iter().map(|k| Expr::Str(k.to_string())).collect();
                cond = Expr::and(cond, Expr::in_list(Expr::col("e", "kind"), kind_exprs));
            }
            cond
        })
        .join(
            TableRef::scan("node", "n"),
            Expr::eq(
                Expr::col("n", "id"),
                Expr::case(
                    Expr::eq(Expr::col("e", "from_id"), Expr::bare("p.cur")),
                    Expr::col("e", "to_id"),
                    Expr::col("e", "from_id"),
                ),
            ),
        )
        .col_as(Expr::bare("p.start_id"), "start_id")
        .col_as(Expr::col("n", "id"), "cur")
        .col_as(
            Expr::func("list_append", vec![Expr::bare("p.visited"), Expr::col("n", "id")]),
            "visited",
        )
        .col_as(
            Expr::func("list_append", vec![
                Expr::bare("p.trail"),
                Expr::func("concat", vec![
                    Expr::col("e", "kind"),
                    Expr::Str(":".into()),
                    Expr::case(
                        Expr::eq(Expr::col("e", "from_id"), Expr::bare("p.cur")),
                        Expr::Str(">".into()),
                        Expr::Str("<".into()),
                    ),
                ]),
            ]),
            "trail",
        )
        .col_as(Expr::bare("p.depth + 1"), "depth")
        .where_and(Expr::lt(Expr::bare("p.depth"), Expr::Int(max_depth as i64)))
        .where_and(Expr::negate(Expr::func(
            "list_contains",
            vec![Expr::bare("p.visited"), Expr::col("n", "id")],
        )));

    let outer = Query::from(TableRef::cte("paths", "r"))
        .col_as(Expr::cast(Expr::bare("visited"), "VARCHAR"), "visited")
        .col_as(Expr::cast(Expr::bare("trail"), "VARCHAR"), "trail")
        .col_bare("depth")
        .where_and(Expr::eq(Expr::bare("r.cur"), Expr::Int(to_id)))
        .order("depth", false)
        .limit(1)
        .with_cte(Cte {
            name: "paths".into(),
            recursive: true,
            base,
            recursive_term: Some(recursive),
        });

    let compiled = outer.build();
    let mut stmt = db.conn().prepare(compiled.sql())?;

    let result = stmt.query_row(compiled.param_refs().as_slice(), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    });

    match result {
        Ok((visited_str, trail_str)) => {
            let visited = parse_int_list(&visited_str);
            let trails = parse_str_list(&trail_str);

            let mut hops = Vec::new();
            for (i, &nid) in visited.iter().enumerate() {
                let node = db.get_node_by_id(nid)?;
                let label = node.label().to_string();
                let kind = node.kind.clone();
                let (edge_kind, direction) = if i > 0 {
                    parse_trail(trails.get(i - 1))
                } else {
                    (None, None)
                };
                hops.push(PathHop { node_id: nid, node_kind: kind, node_label: label, edge_kind, direction });
            }
            Ok(Some(PathResult { hops }))
        }
        Err(e) if e.to_string().contains("no rows") => Ok(None),
        Err(e) => Err(DbError::DuckDb(e)),
    }
}

fn parse_trail(entry: Option<&String>) -> (Option<String>, Option<Direction>) {
    let s = match entry { Some(s) => s, None => return (None, None) };
    match s.rsplit_once(':') {
        Some((ek, ">")) => (Some(ek.into()), Some(Direction::Outgoing)),
        Some((ek, "<")) => (Some(ek.into()), Some(Direction::Incoming)),
        _ => (None, None),
    }
}

fn parse_int_list(s: &str) -> Vec<i64> {
    s.trim_matches(&['[', ']'][..]).split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn parse_str_list(s: &str) -> Vec<String> {
    let t = s.trim_matches(&['[', ']'][..]);
    if t.is_empty() { return vec![]; }
    t.split(',').map(|v| v.trim().trim_matches('\'').into()).collect()
}
