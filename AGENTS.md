# ctx

Local knowledge graph CLI for AI coding sessions. Rust + DuckDB.

## Layout

- `SPEC.md` — full specification (data model, CLI commands, schema system, output formats)
- `Cargo.toml` — workspace root (shared deps, edition, lints)
- `rust-toolchain.toml` — pinned stable toolchain
- `deny.toml` — cargo-deny license/dependency audit
- `config/schema.toml` — default schema (embedded into binary, overridable at runtime)

## Crates

```
crates/
├── cli/       — binary: clap subcommands, output formatting, dispatch
├── db/        — DuckDB connection, node/edge CRUD, typed SQL AST + codegen, retry logic
├── schema/    — schema.toml parsing, type validation, view DDL generation
└── graph/     — walk/path traversal, edge path parsing, FTS search, recursive CTEs
```

## Dependency flow

```
schema
  │
  v
  db (sql AST, codegen, CRUD)
  │
  v
graph (traverse, path, search)
  │
  v
 cli
```

## SQL layer (db/src/sql.rs)

All SQL is built via a typed AST: `Expr`, `Op`, `TableRef`, `Query`, `Cte`.
Builder methods on `Query` (`.col()`, `.join()`, `.where_and()`, `.limit()`, `.with_cte()`).
`Query::build()` compiles to `CompiledQuery` (sql string + bound params).
No raw SQL string interpolation with user values anywhere.

## Key concepts

- `~/.ctx/ctx.db` — DuckDB database (nodes + edges)
- `~/.ctx/schema.toml` — node/edge type definitions with typed properties
- `Kind:id` refs (e.g., `Session:1`, `Topic:3`)
- Edge paths for traversal: `HAS_TOPIC/~HAS_TOPIC` (slash-delimited, `~` reverses direction)
- Filter micro-syntax: `key=value`, `key>value`, `key~value` (contains), `key^value` (starts with)

## Commands

Nodes: `add`, `get`, `set`, `rm`
Edges: `link`, `unlink`, `edges`
Query: `find`, `search`, `count`
Traversal: `walk`, `path`
Session: `register`, `sessions`, `related`, `timeline`
Schema: `define`, `extend`, `schema`, `kinds`
Meta: `stats`, `export`, `import`, `reindex`
