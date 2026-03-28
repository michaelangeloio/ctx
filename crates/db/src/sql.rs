use crate::DbError;

// ---------------------------------------------------------------------------
// Param — bound query parameter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Param {
    Str(String),
    Int(i64),
}

impl Param {
    pub fn to_duckdb(&self) -> &dyn duckdb::ToSql {
        match self {
            Param::Str(v) => v,
            Param::Int(v) => v,
        }
    }
}

// ---------------------------------------------------------------------------
// CompiledQuery — executable SQL + params
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CompiledQuery {
    text: String,
    params: Vec<Param>,
}

impl CompiledQuery {
    pub fn new(text: String, params: Vec<Param>) -> Self {
        Self { text, params }
    }

    pub fn sql(&self) -> &str {
        &self.text
    }

    pub fn param_refs(&self) -> Vec<&dyn duckdb::ToSql> {
        self.params.iter().map(|p| p.to_duckdb()).collect()
    }
}

// ---------------------------------------------------------------------------
// Op — SQL operators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum Op {
    Eq, Ne, Lt, Le, Gt, Ge, And, Or, ILike, In, IsNull, IsNotNull,
}

impl Op {
    fn as_str(self) -> &'static str {
        match self {
            Op::Eq => "=", Op::Ne => "!=",
            Op::Lt => "<", Op::Le => "<=", Op::Gt => ">", Op::Ge => ">=",
            Op::And => "AND", Op::Or => "OR",
            Op::ILike => "ILIKE", Op::In => "IN",
            Op::IsNull => "IS NULL", Op::IsNotNull => "IS NOT NULL",
        }
    }
}

// ---------------------------------------------------------------------------
// Expr — typed expression tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Expr {
    Col { table: String, col: String },
    Str(String),
    Int(i64),
    BinOp { op: Op, left: Box<Expr>, right: Box<Expr> },
    Postfix { op: Op, expr: Box<Expr> },
    Not(Box<Expr>),
    Func { name: String, args: Vec<Expr> },
    Case { when: Box<Expr>, then: Box<Expr>, else_: Box<Expr> },
    Cast { expr: Box<Expr>, to: String },
    InSubquery { expr: Box<Expr>, subquery: Box<Query> },
    Star,
    /// A list literal like [id] or [].
    List(Vec<Expr>),
    /// A bare column name without table prefix (for CTE references).
    Bare(String),
}

impl Expr {
    pub fn col(table: &str, col: &str) -> Self {
        Self::Col { table: table.into(), col: col.into() }
    }

    pub fn bare(name: &str) -> Self {
        Self::Bare(name.into())
    }

    pub fn eq(l: Expr, r: Expr) -> Self {
        Self::BinOp { op: Op::Eq, left: l.into(), right: r.into() }
    }

    pub fn ne(l: Expr, r: Expr) -> Self {
        Self::BinOp { op: Op::Ne, left: l.into(), right: r.into() }
    }

    pub fn lt(l: Expr, r: Expr) -> Self {
        Self::BinOp { op: Op::Lt, left: l.into(), right: r.into() }
    }

    pub fn and(l: Expr, r: Expr) -> Self {
        Self::BinOp { op: Op::And, left: l.into(), right: r.into() }
    }

    pub fn or(l: Expr, r: Expr) -> Self {
        Self::BinOp { op: Op::Or, left: l.into(), right: r.into() }
    }

    pub fn ilike(l: Expr, r: Expr) -> Self {
        Self::BinOp { op: Op::ILike, left: l.into(), right: r.into() }
    }

    pub fn in_list(expr: Expr, values: Vec<Expr>) -> Self {
        Self::BinOp { op: Op::In, left: expr.into(), right: Expr::List(values).into() }
    }

    pub fn negate(expr: Expr) -> Self {
        Self::Not(expr.into())
    }

    pub fn case(when: Expr, then: Expr, else_: Expr) -> Self {
        Self::Case { when: when.into(), then: then.into(), else_: else_.into() }
    }

    pub fn cast(expr: Expr, to: &str) -> Self {
        Self::Cast { expr: expr.into(), to: to.into() }
    }

    pub fn func(name: &str, args: Vec<Expr>) -> Self {
        Self::Func { name: name.into(), args }
    }

    pub fn concat(parts: Vec<Expr>) -> Self {
        if parts.len() == 1 { return parts.into_iter().next().unwrap(); }
        // a || b || c
        parts.into_iter().reduce(|a, b| {
            Expr::BinOp { op: Op::Eq, left: a.into(), right: b.into() }
        }).unwrap()
    }

    pub fn and_all(exprs: impl IntoIterator<Item = Expr>) -> Option<Expr> {
        exprs.into_iter().reduce(Expr::and)
    }

    pub fn or_all(exprs: impl IntoIterator<Item = Expr>) -> Option<Expr> {
        exprs.into_iter().reduce(Expr::or)
    }
}

// ---------------------------------------------------------------------------
// TableRef — FROM clause with JOINs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum TableRef {
    Scan { table: String, alias: String },
    Join { left: Box<TableRef>, right: Box<TableRef>, on: Expr },
    CteRef { name: String, alias: String },
}

impl TableRef {
    pub fn scan(table: &str, alias: &str) -> Self {
        Self::Scan { table: table.into(), alias: alias.into() }
    }

    pub fn join(left: TableRef, right: TableRef, on: Expr) -> Self {
        Self::Join { left: left.into(), right: right.into(), on }
    }

    pub fn cte(name: &str, alias: &str) -> Self {
        Self::CteRef { name: name.into(), alias: alias.into() }
    }
}

// ---------------------------------------------------------------------------
// Cte — WITH clause entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Cte {
    pub name: String,
    pub recursive: bool,
    pub base: Query,
    pub recursive_term: Option<Query>,
}

// ---------------------------------------------------------------------------
// Query — full SELECT statement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Query {
    pub ctes: Vec<Cte>,
    pub distinct: bool,
    pub columns: Vec<(Expr, Option<String>)>,
    pub from: TableRef,
    pub filter: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub order_by: Vec<(String, bool)>,
    pub limit: Option<i64>,
}

impl Query {
    pub fn from(table: TableRef) -> Self {
        Self {
            ctes: Vec::new(),
            distinct: false,
            columns: Vec::new(),
            from: table,
            filter: None,
            group_by: Vec::new(),
            order_by: Vec::new(),
            limit: None,
        }
    }

    pub fn distinct(mut self) -> Self { self.distinct = true; self }

    pub fn col(mut self, table: &str, col: &str) -> Self {
        self.columns.push((Expr::col(table, col), None));
        self
    }

    pub fn cols(mut self, table: &str, cols: &[&str]) -> Self {
        for c in cols { self.columns.push((Expr::col(table, c), None)); }
        self
    }

    pub fn col_as(mut self, expr: Expr, alias: &str) -> Self {
        self.columns.push((expr, Some(alias.into())));
        self
    }

    pub fn col_bare(mut self, name: &str) -> Self {
        self.columns.push((Expr::bare(name), None));
        self
    }

    pub fn join(mut self, right: TableRef, on: Expr) -> Self {
        self.from = TableRef::join(self.from, right, on);
        self
    }

    pub fn where_and(mut self, expr: Expr) -> Self {
        self.filter = Some(match self.filter {
            Some(existing) => Expr::and(existing, expr),
            None => expr,
        });
        self
    }

    pub fn group(mut self, expr: Expr) -> Self {
        self.group_by.push(expr);
        self
    }

    pub fn order(mut self, col_ref: &str, desc: bool) -> Self {
        self.order_by.push((col_ref.into(), desc));
        self
    }

    pub fn limit(mut self, n: i64) -> Self { self.limit = Some(n); self }

    pub fn with_cte(mut self, cte: Cte) -> Self {
        self.ctes.push(cte);
        self
    }

    /// Compile to executable SQL + params.
    pub fn build(self) -> CompiledQuery {
        let mut out = String::new();
        let mut params = Vec::new();
        emit_query(&self, &mut out, &mut params);
        CompiledQuery::new(out, params)
    }
}

// ---------------------------------------------------------------------------
// Codegen — recursive tree walk
// ---------------------------------------------------------------------------

fn emit_query(q: &Query, out: &mut String, params: &mut Vec<Param>) {
    // CTEs
    if !q.ctes.is_empty() {
        out.push_str("WITH ");
        for (i, cte) in q.ctes.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            if cte.recursive { out.push_str("RECURSIVE "); }
            out.push_str(&cte.name);
            out.push_str(" AS (");
            emit_query(&cte.base, out, params);
            if let Some(rec) = &cte.recursive_term {
                out.push_str(" UNION ALL ");
                emit_query(rec, out, params);
            }
            out.push(')');
        }
        out.push(' ');
    }

    out.push_str("SELECT ");
    if q.distinct { out.push_str("DISTINCT "); }

    if q.columns.is_empty() {
        out.push('*');
    } else {
        for (i, (expr, alias)) in q.columns.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            emit_expr(expr, out, params);
            if let Some(a) = alias { out.push_str(" AS "); out.push_str(a); }
        }
    }

    out.push_str(" FROM ");
    emit_table_ref(&q.from, out, params);

    if let Some(f) = &q.filter {
        out.push_str(" WHERE ");
        emit_expr(f, out, params);
    }

    if !q.group_by.is_empty() {
        out.push_str(" GROUP BY ");
        for (i, expr) in q.group_by.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            emit_expr(expr, out, params);
        }
    }

    if !q.order_by.is_empty() {
        out.push_str(" ORDER BY ");
        for (i, (col, desc)) in q.order_by.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(col);
            if *desc { out.push_str(" DESC"); }
        }
    }

    if let Some(n) = q.limit {
        out.push_str(" LIMIT ");
        out.push_str(&n.to_string());
    }
}

fn emit_expr(expr: &Expr, out: &mut String, params: &mut Vec<Param>) {
    match expr {
        Expr::Col { table, col } => {
            out.push_str(table); out.push('.'); out.push_str(col);
        }
        Expr::Bare(name) => out.push_str(name),
        Expr::Str(v) => { out.push('?'); params.push(Param::Str(v.clone())); }
        Expr::Int(v) => { out.push('?'); params.push(Param::Int(*v)); }
        Expr::BinOp { op, left, right } => {
            out.push('(');
            emit_expr(left, out, params);
            out.push(' '); out.push_str(op.as_str()); out.push(' ');
            emit_expr(right, out, params);
            out.push(')');
        }
        Expr::Postfix { op, expr } => {
            out.push('(');
            emit_expr(expr, out, params);
            out.push(' '); out.push_str(op.as_str());
            out.push(')');
        }
        Expr::Not(inner) => {
            out.push_str("NOT ");
            emit_expr(inner, out, params);
        }
        Expr::Func { name, args } => {
            out.push_str(name); out.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                emit_expr(arg, out, params);
            }
            out.push(')');
        }
        Expr::Case { when, then, else_ } => {
            out.push_str("CASE WHEN ");
            emit_expr(when, out, params);
            out.push_str(" THEN ");
            emit_expr(then, out, params);
            out.push_str(" ELSE ");
            emit_expr(else_, out, params);
            out.push_str(" END");
        }
        Expr::Cast { expr, to } => {
            out.push_str("CAST(");
            emit_expr(expr, out, params);
            out.push_str(" AS "); out.push_str(to); out.push(')');
        }
        Expr::InSubquery { expr, subquery } => {
            emit_expr(expr, out, params);
            out.push_str(" IN (");
            emit_query(subquery, out, params);
            out.push(')');
        }
        Expr::Star => out.push('*'),
        Expr::List(items) => {
            out.push('(');
            for (i, item) in items.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                emit_expr(item, out, params);
            }
            out.push(')');
        }
    }
}

fn emit_table_ref(tr: &TableRef, out: &mut String, params: &mut Vec<Param>) {
    match tr {
        TableRef::Scan { table, alias } => {
            out.push_str(table); out.push_str(" AS "); out.push_str(alias);
        }
        TableRef::Join { left, right, on } => {
            emit_table_ref(left, out, params);
            out.push_str(" INNER JOIN ");
            emit_table_ref(right, out, params);
            out.push_str(" ON ");
            emit_expr(on, out, params);
        }
        TableRef::CteRef { name, alias } => {
            out.push_str(name); out.push_str(" AS "); out.push_str(alias);
        }
    }
}

// ---------------------------------------------------------------------------
// Identifier validation
// ---------------------------------------------------------------------------

pub fn require_ident(s: &str) -> Result<&str, DbError> {
    if ctx_schema::is_safe_identifier(s) { Ok(s) }
    else { Err(DbError::NotFound(format!("invalid identifier '{s}'"))) }
}

// ---------------------------------------------------------------------------
// Column lists
// ---------------------------------------------------------------------------

pub const NODE_COLS: &[&str] = &[
    "id", "kind", "properties", "created_at::VARCHAR", "updated_at::VARCHAR",
];

pub const EDGE_COLS: &[&str] = &[
    "id", "kind", "from_id", "to_id", "from_kind", "to_kind", "properties", "created_at::VARCHAR",
];

pub fn node_cols_csv() -> String { NODE_COLS.join(", ") }
pub fn edge_cols_csv() -> String { EDGE_COLS.join(", ") }

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_select() {
        let q = Query::from(TableRef::scan("node", "n"))
            .cols("n", &["id", "kind"])
            .where_and(Expr::eq(Expr::col("n", "kind"), Expr::Str("Session".into())))
            .limit(10)
            .build();

        assert!(q.sql().contains("n.id, n.kind"));
        assert!(q.sql().contains("FROM node AS n"));
        assert!(q.sql().contains("WHERE (n.kind = ?)"));
        assert_eq!(q.params.len(), 1);
    }

    #[test]
    fn join_chain() {
        let q = Query::from(TableRef::scan("node", "n0"))
            .join(
                TableRef::scan("edge", "e0"),
                Expr::and(
                    Expr::eq(Expr::col("n0", "id"), Expr::col("e0", "from_id")),
                    Expr::eq(Expr::col("e0", "kind"), Expr::Str("HAS_TOPIC".into())),
                ),
            )
            .join(
                TableRef::scan("node", "n1"),
                Expr::eq(Expr::col("e0", "to_id"), Expr::col("n1", "id")),
            )
            .cols("n1", &["id", "kind"])
            .where_and(Expr::eq(Expr::col("n0", "id"), Expr::Int(1)))
            .limit(30)
            .build();

        assert!(q.sql().contains("INNER JOIN edge AS e0"));
        assert!(q.sql().contains("INNER JOIN node AS n1"));
        assert_eq!(q.params.len(), 2);
    }

    #[test]
    fn cte_query() {
        let base = Query::from(TableRef::scan("node", "n"))
            .col_as(Expr::col("n", "id"), "nid")
            .where_and(Expr::eq(Expr::col("n", "id"), Expr::Int(1)));

        let outer = Query::from(TableRef::cte("my_cte", "c"))
            .col("c", "nid")
            .with_cte(Cte {
                name: "my_cte".into(),
                recursive: false,
                base,
                recursive_term: None,
            })
            .build();

        assert!(outer.sql().contains("WITH my_cte AS ("));
        assert!(outer.sql().contains("FROM my_cte AS c"));
    }
}
