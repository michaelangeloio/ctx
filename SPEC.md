# ctx — local knowledge graph for AI coding sessions

## Overview

`ctx` is a CLI that builds and queries a local property graph. Agents call `ctx` during coding sessions to record what they worked on, tag topics, link related sessions, and pull up prior context. The graph persists across sessions and across tools (Claude Code, Codex, etc.).

The primary consumer is an LLM agent in a terminal. The design priorities:

- Token efficiency: commands are short, output is compact
- Schema safety: agents cannot invent properties; the schema defines what exists
- Query speed: integer primary keys, DuckDB columnar engine, indexed edges
- Graph-native: edges and traversal are first-class

## Architecture

```
Agent (LLM in terminal)
  │
  ctx add Session title="Fix JWT" tool=claude ...
  ctx link Session:1 HAS_TOPIC Topic:3
  ctx walk Session:1 HAS_TOPIC/~HAS_TOPIC
  ctx search "JWT validation"
  │
  ├── CLI (argument parsing, filter tokenizer)
  ├── Schema Validator (schema.toml → property/type/edge checks)
  ├── Query Planner (translate commands → SQL)
  └── DuckDB (embedded, single file)
       └── ~/.ctx/ctx.db
```

### Why DuckDB

Single-file embedded database with no external dependencies. Columnar engine, so aggregation over millions of rows is fast. Has native JSON extraction, an FTS extension, and recursive CTEs. Parquet export/import for backup. Integer joins are hardware-optimized at scale.

## Storage layout

```
~/.ctx/
├── ctx.db              # DuckDB database (all nodes + edges)
├── schema.toml         # Node/edge type definitions
└── backups/            # Parquet exports
    └── 2026-03-28.parquet
```

Override the database location with `CTX_DB` env var or `--db <path>`.

## Data model

### Tables

Three tables.

```sql
CREATE TABLE node (
  id          INTEGER PRIMARY KEY DEFAULT nextval('node_id_seq'),
  kind        VARCHAR NOT NULL,     -- e.g. 'Session', 'Topic'
  properties  JSON    NOT NULL DEFAULT '{}',
  created_at  TIMESTAMP NOT NULL DEFAULT now(),
  updated_at  TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE edge (
  id       INTEGER PRIMARY KEY DEFAULT nextval('edge_id_seq'),
  kind     VARCHAR NOT NULL,     -- e.g. 'HAS_TOPIC', 'CONTINUES'
  from_id  INTEGER NOT NULL REFERENCES node(id) ON DELETE CASCADE,
  to_id    INTEGER NOT NULL REFERENCES node(id) ON DELETE CASCADE,
  properties JSON NOT NULL DEFAULT '{}',
  created_at TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE schema_registry (
  kind       VARCHAR PRIMARY KEY,
  category   VARCHAR NOT NULL,   -- 'node' or 'edge'
  definition JSON    NOT NULL    -- parsed schema.toml entry
);
```

### Indexes

```sql
CREATE INDEX idx_node_kind ON node(kind);
CREATE INDEX idx_edge_from ON edge(from_id, kind);
CREATE INDEX idx_edge_to   ON edge(to_id, kind);
CREATE INDEX idx_edge_kind ON edge(kind);
```

### Views

Each node kind gets an auto-generated view that extracts JSON properties into typed columns:

```sql
-- auto-generated from schema.toml
CREATE VIEW v_session AS
SELECT
  id,
  properties->>'session_id'   AS session_id,
  properties->>'title'        AS title,
  properties->>'tool'         AS tool,
  properties->>'model'        AS model,
  properties->>'project_path' AS project_path,
  CAST(properties->>'message_count' AS INTEGER) AS message_count,
  properties->>'summary'      AS summary,
  properties->>'git_branch'   AS git_branch,
  properties->>'worktree_path' AS worktree_path,
  properties->>'filepath'     AS filepath,
  properties->>'first_prompt' AS first_prompt,
  created_at,
  updated_at
FROM node WHERE kind = 'Session';
```

DuckDB can push predicates into these views, so queries on typed columns avoid runtime JSON extraction.

## Schema system

### schema.toml

Defines all legal node kinds with typed properties, and all legal edge kinds with endpoint constraints.

```toml
# ~/.ctx/schema.toml

# ──────────────────────────────────────
# Node kinds
# ──────────────────────────────────────

[nodes.Session]
session_id    = "string"
title         = "string"
summary       = "string?"
tool          = "enum:claude,codex"
model         = "string"
message_count = "int?"
project_path  = "string"
git_branch    = "string?"
worktree_path = "string?"
filepath      = "string?"
first_prompt  = "string?"

[nodes.Project]
name       = "string"
path       = "string"
remote_url = "string?"

[nodes.Topic]
name = "string"

[nodes.Branch]
name = "string"

[nodes.Model]
name     = "string"
provider = "enum:anthropic,openai"

# ──────────────────────────────────────
# Edge kinds
# ──────────────────────────────────────

[edges.IN_PROJECT]
from = ["Session"]
to   = ["Project"]

[edges.ON_BRANCH]
from = ["Session"]
to   = ["Branch"]

[edges.HAS_TOPIC]
from = ["Session"]
to   = ["Topic"]

[edges.USED_MODEL]
from = ["Session"]
to   = ["Model"]

[edges.CONTINUES]
from = ["Session"]
to   = ["Session"]
# edge properties:
reason = "string?"

[edges.SPAWNED]
from = ["Session"]
to   = ["Session"]

[edges.RELATED_TO]
from = ["Session"]
to   = ["Session"]
reason = "string?"
```

### Type system

| Syntax          | Meaning                      | DuckDB type   | Example                        |
|-----------------|------------------------------|---------------|--------------------------------|
| `string`        | Required string              | `VARCHAR`     | `title = "string"`             |
| `string?`       | Optional string              | `VARCHAR`     | `summary = "string?"`          |
| `int`           | Required integer             | `INTEGER`     | `count = "int"`                |
| `int?`          | Optional integer             | `INTEGER`     | `message_count = "int?"`       |
| `float`         | Required float               | `DOUBLE`      | `score = "float"`              |
| `float?`        | Optional float               | `DOUBLE`      | `duration = "float?"`          |
| `bool`          | Required boolean             | `BOOLEAN`     | `archived = "bool"`            |
| `bool?`         | Optional boolean             | `BOOLEAN`     | `draft = "bool?"`              |
| `timestamp`     | Required timestamp           | `TIMESTAMP`   | `started = "timestamp"`        |
| `timestamp?`    | Optional timestamp           | `TIMESTAMP`   | `finished = "timestamp?"`      |
| `enum:a,b,c`    | Required, one of the values  | `VARCHAR`     | `tool = "enum:claude,codex"`   |
| `enum:a,b,c?`   | Optional enum                | `VARCHAR`     | `status = "enum:open,closed?"` |

`?` means optional (nullable). Without it, the property is required at creation time.

### Implicit properties

Every node gets these automatically. They never appear in `schema.toml`:

| Property     | Type        | Set by        |
|--------------|-------------|---------------|
| `id`         | `int`       | Auto-increment |
| `created_at` | `timestamp` | Auto on create |
| `updated_at` | `timestamp` | Auto on create/update |

### Validation rules

1. `ctx add <Kind>` — all required properties must be present, no unknown properties, enum values checked, types coerced
2. `ctx set <ref>` — only known properties, types checked, required properties cannot be nulled
3. `ctx link <ref> <EdgeKind> <ref>` — edge kind must exist in schema, from/to node kinds must match the schema's `from`/`to` arrays, edge properties validated the same way
4. `ctx define` / `ctx extend` — writes to `schema.toml`, regenerates views

## CLI reference

### Node operations

```
ctx add <Kind> [key=value...]
```

Create a node. Returns the integer ID.

```bash
$ ctx add Session title="Fix JWT" tool=claude model=opus project_path=/app
Session:1
```

---

```
ctx get <ref>
```

Show a node and all its edges. This is the one command where full output is the default, since you asked for a specific node.

```bash
$ ctx get Session:1
Session:1 "Fix JWT"
  session_id=dba27d8f  tool=claude  model=opus  project_path=/app  created=2026-03-28
  > HAS_TOPIC: Topic:3 "auth", Topic:7 "security"
  > IN_PROJECT: Project:1 "/app"
  > CONTINUES: Session:4 "Follow-up JWT work"
  < RELATED_TO: Session:12 "Auth refactor"
```

The ref format is `Kind:id` (e.g., `Session:1`). Prefix matching is supported: `Session:1` matches `Session:1`, `Session:10`, etc. If ambiguous, returns an error.

---

```
ctx set <ref> key=value...
```

Merge properties onto an existing node. Only updates the keys you pass.

```bash
$ ctx set Session:1 summary="Fixed JWT validation in auth middleware" message_count=42
```

---

```
ctx rm <ref>
```

Delete a node and cascade-delete all its edges.

```bash
$ ctx rm Session:1
```

### Edge operations

```
ctx link <ref> <EdgeKind> <ref> [key=value...]
```

Create a directed edge. Optional edge properties.

```bash
$ ctx link Session:1 HAS_TOPIC Topic:3
$ ctx link Session:1 CONTINUES Session:4 reason="same bug"
```

---

```
ctx unlink <ref> <EdgeKind> <ref>
```

Remove an edge.

```bash
$ ctx unlink Session:1 HAS_TOPIC Topic:3
```

---

```
ctx edges <ref> [--in|--out] [--kind <EdgeKind>]
```

List edges on a node.

```bash
$ ctx edges Session:1
> HAS_TOPIC: Topic:3 "auth", Topic:7 "security"
> IN_PROJECT: Project:1 "/app"
> CONTINUES: Session:4 "Follow-up JWT work"
< RELATED_TO: Session:12 "Auth refactor"

$ ctx edges Session:1 --out --kind HAS_TOPIC
> HAS_TOPIC: Topic:3 "auth", Topic:7 "security"
```

### Query operations

```
ctx find <Kind> [key=value...] [--limit N] [--order <prop>[:asc|:desc]]
```

Filter nodes by kind and property values. Uses the filter micro-syntax (see below).

```bash
$ ctx find Session tool=claude,project_path=/app --limit 5 --order created_at:desc
Session:1 "Fix JWT"
Session:8 "Add rate limiting"
Session:14 "Refactor middleware"
```

---

```
ctx search <text> [--kind <Kind>] [--limit N]
```

Full-text search across all string properties. Combines BM25, fuzzy matching (Jaro-Winkler), and exact substring via reciprocal rank fusion.

```bash
$ ctx search "JWT validation" --kind Session --limit 5
Session:1 "Fix JWT" (0.92)
Session:14 "Refactor middleware" (0.41)
```

---

```
ctx count <Kind> [--by <prop>] [--where key=value...]
```

Count nodes, optionally grouped.

```bash
$ ctx count Session --by tool

tool     count
claude   142
codex    87

$ ctx count Session --by model --where tool=claude

model    count
opus     98
sonnet   44
```

### Graph traversal

```
ctx walk <ref> <edge-path> [--where key=value...] [--limit N]
```

Traverse the graph along an edge path. Edge paths are `/`-delimited edge kinds. `~` reverses direction.

```bash
# 1 hop: sessions → topics
$ ctx walk Session:1 HAS_TOPIC
Topic:3 "auth"
Topic:7 "security"

# 2 hops: session → topics → back to sessions with same topics
$ ctx walk Session:1 HAS_TOPIC/~HAS_TOPIC
Session:8 "Add rate limiting"
Session:12 "Auth refactor"
Session:19 "Security audit"

# 3 hops: session → project → back to sessions in same project → their topics
$ ctx walk Session:1 IN_PROJECT/~IN_PROJECT/HAS_TOPIC
Topic:3 "auth"
Topic:12 "database"
Topic:15 "api"

# wildcard: any edge, 1 hop
$ ctx walk Session:1 *
Topic:3 "auth"
Topic:7 "security"
Project:1 "myapp"
Session:4 "Follow-up JWT work"

# filter results
$ ctx walk Session:1 HAS_TOPIC/~HAS_TOPIC --where tool=codex
Session:19 "Security audit"
```

---

```
ctx path <ref> <ref> [--depth N] [--via <EdgeKind>,...]
```

Shortest path between two nodes. Default max depth is 3.

```bash
$ ctx path Session:1 Session:42 --depth 3
Session:1 >HAS_TOPIC> Topic:3 <HAS_TOPIC< Session:42  (2 hops)

$ ctx path Session:1 Session:99 --depth 3 --via HAS_TOPIC,CONTINUES
No path found within depth 3.
```

### Edge path syntax

`/`-delimited edge kind references. Each step becomes a JOIN against the `edge` table with the appropriate `from_id`/`to_id` direction.

| Syntax          | Meaning                                      |
|-----------------|----------------------------------------------|
| `HAS_TOPIC`     | Follow HAS_TOPIC outward (1 hop)             |
| `~HAS_TOPIC`    | Follow HAS_TOPIC inward (reverse, 1 hop)     |
| `A/B`           | Follow A then B (2 hops)                     |
| `A/~B`          | Follow A outward, then B inward (2 hops)     |
| `A/B/C`         | 3 hops                                       |
| `*`             | Any edge kind outward (1 hop)                |
| `~*`            | Any edge kind inward (1 hop)                 |
| `*/HAS_TOPIC`   | Any edge then HAS_TOPIC (2 hops)             |

Maximum depth: 5 hops (configurable via `--depth`).

### Filter micro-syntax

Used in `--where` flags and inline `key=value` arguments on `ctx find`.

| Syntax      | Meaning        | SQL equivalent           |
|-------------|----------------|--------------------------|
| `key=value` | Equals         | `key = 'value'`          |
| `key!=value`| Not equals     | `key != 'value'`         |
| `key>value` | Greater than   | `key > value`            |
| `key<value` | Less than      | `key < value`            |
| `key>=value`| Greater/equal  | `key >= value`           |
| `key<=value`| Less/equal     | `key <= value`           |
| `key~value` | Contains       | `key ILIKE '%value%'`    |
| `key^value` | Starts with    | `key ILIKE 'value%'`     |

Comma-separated for AND:

```bash
ctx find Session tool=claude,created_at>2026-03-01 --limit 10
```

The parser is about 30 lines: split on `,`, match the operator with `^(\w+)(!=|>=|<=|>|<|~|\^|=)(.+)$`, extract key/op/value.

### Session commands

Convenience commands built on the graph primitives.

```
ctx register [--tool <tool>] [--session-id <uuid>] [--model <model>]
             [--project <path>] [--branch <branch>] [--title <title>]
```

Register the current session mid-flight. The agent calls this to record itself. Creates a `Session` node and auto-links to `Project`/`Branch`/`Model` nodes, creating them if they don't exist.

```bash
$ ctx register --tool claude --session-id $SESSION_ID --model opus \
    --project /app --branch fix/jwt --title "Fix JWT validation"
Session:231
```

---

```
ctx sessions [--tool claude|codex] [--project <path>] [--branch <branch>] [--limit N]
```

List sessions. Shortcut for `ctx find Session` with common filters.

```bash
$ ctx sessions --project /app --limit 5
Session:231 "Fix JWT validation"
Session:220 "Add rate limiting"
Session:198 "DB migration"
```

---

```
ctx related <ref> [--limit N]
```

Find sessions that share topics with the given session. Shortcut for `ctx walk <ref> HAS_TOPIC/~HAS_TOPIC`.

```bash
$ ctx related Session:231
Session:198 "DB migration"
Session:180 "Auth middleware"
Session:145 "Security audit"
```

---

```
ctx timeline [--project <path>] [--last N]
```

Chronological session list for a project, with continuation chains.

```bash
$ ctx timeline --project /app --last 10
2026-03-28  Session:231 "Fix JWT validation"
             └── Session:220
2026-03-25  Session:220 "Add rate limiting"
2026-03-22  Session:198 "DB migration"
2026-03-20  Session:190 "Auth middleware"
             └── Session:180
```

### Schema management

```
ctx define <Kind> [prop:type...]
```

Define a new node kind. Writes to `schema.toml` and regenerates the view.

```bash
$ ctx define Snippet language:string content:string source:string? tags:string?
Defined node kind 'Snippet' with 4 properties.
```

---

```
ctx extend <Kind> [prop:type...]
```

Add optional properties to an existing kind. Only `?` (optional) types allowed, since you can't add required properties to a kind that already has data.

```bash
$ ctx extend Session cli_version:string?
Added 1 property to 'Session'.
```

---

```
ctx schema [Kind]
```

Show the schema. Without arguments, lists all kinds. With a kind name, shows properties and valid edges.

```bash
$ ctx schema Session
Session (5 required, 5 optional)
  session_id     string
  title          string
  tool           enum: claude, codex
  model          string
  project_path   string
  summary        string?
  message_count  int?
  git_branch     string?
  worktree_path  string?
  filepath       string?
  first_prompt   string?

  edges out: IN_PROJECT → Project
             ON_BRANCH → Branch
             HAS_TOPIC → Topic
             USED_MODEL → Model
             CONTINUES → Session
             SPAWNED → Session
             RELATED_TO → Session
  edges in:  CONTINUES ← Session
             SPAWNED ← Session
             RELATED_TO ← Session

$ ctx schema
Nodes: Session, Project, Topic, Branch, Model
Edges: IN_PROJECT, ON_BRANCH, HAS_TOPIC, USED_MODEL, CONTINUES, SPAWNED, RELATED_TO
```

---

```
ctx kinds
```

List registered node kinds with counts.

```bash
$ ctx kinds
Session   229
Project    12
Topic      45
Branch     34
Model       4
```

### Meta operations

```
ctx stats
```

Database statistics.

```bash
$ ctx stats
Nodes: 324  (Session: 229, Project: 12, Topic: 45, Branch: 34, Model: 4)
Edges: 891  (HAS_TOPIC: 412, IN_PROJECT: 229, ON_BRANCH: 142, ...)
DB size: 2.4 MB
```

---

```
ctx export [--format parquet|json] [--output <path>]
```

Export the database.

```bash
$ ctx export --format parquet --output backup.parquet
```

---

```
ctx import <path>
```

Import from a previous export.

```bash
$ ctx import backup.parquet
```

---

```
ctx reindex
```

Rebuild the FTS index. Run after bulk imports.

```bash
$ ctx reindex
Rebuilt FTS index: 324 nodes indexed.
```

## Output formats

### Default: compact

One node per line. Ref + label.

```bash
$ ctx find Session tool=claude --limit 3
Session:1 "Fix JWT"
Session:8 "Add rate limiting"
Session:14 "Refactor middleware"
```

### --full

Properties and edges.

```bash
$ ctx find Session tool=claude --limit 1 --full

Session:1 "Fix JWT"
  session_id=dba27d8f  tool=claude  model=opus  project_path=/app  created=2026-03-28
  > HAS_TOPIC: Topic:3 "auth", Topic:7 "security"
  > IN_PROJECT: Project:1 "/app"
```

### --json

```bash
$ ctx find Session tool=claude --limit 2 --json
[
  {"id": 1, "kind": "Session", "title": "Fix JWT", "tool": "claude", "model": "opus", "created_at": "2026-03-28T12:00:00Z"},
  {"id": 8, "kind": "Session", "title": "Add rate limiting", "tool": "claude", "model": "opus", "created_at": "2026-03-25T09:00:00Z"}
]
```

### --quiet

IDs only.

```bash
$ ctx find Session tool=claude --limit 2 --quiet
1
8
```

## Full-text search

Search combines three signals via reciprocal rank fusion (RRF):

1. BM25 via DuckDB's FTS extension (`PRAGMA create_fts_index`) over all string properties
2. Jaro-Winkler fuzzy similarity on title/name/summary
3. `ILIKE` exact substring on all string properties

Weights: BM25 = 2.0, exact = 1.5, fuzzy = 1.0, k = 60.

The FTS index rebuilds on `ctx reindex`.

## Agent integration

### Skill definition (Claude Code)

Agents discover `ctx` through a skill file:

```markdown
# .claude/skills/ctx/SKILL.md

Use `ctx` to record and query your session knowledge graph.

## When to use
- At session start: register yourself with `ctx register`
- When you discover a topic: tag it with `ctx link`
- When continuing prior work: `ctx related` or `ctx search` for context
- When linking sessions: `ctx link Session:N CONTINUES Session:M`

## Allowed tools
Bash(ctx *)
```

### Agent workflow

```bash
# 1. Register this session
ctx register --tool claude --session-id $SID --model opus \
  --project /app --branch fix/jwt --title "Fix JWT validation"

# 2. Get context from prior sessions
ctx search "JWT"
ctx related Session:231
ctx walk Session:231 CONTINUES

# 3. Tag topics as you work
ctx link Session:231 HAS_TOPIC Topic:3          # existing topic
ctx add Topic name=jwt                           # new topic
ctx link Session:231 HAS_TOPIC Topic:46          # link it

# 4. Link to prior sessions
ctx link Session:231 CONTINUES Session:220 reason="same bug"

# 5. Update when done
ctx set Session:231 summary="Fixed JWT validation in auth middleware" message_count=47
```

### Environment variables

| Variable    | Default             | Description              |
|-------------|---------------------|--------------------------|
| `CTX_DB`    | `~/.ctx/ctx.db`     | Database file path       |
| `CTX_SCHEMA`| `~/.ctx/schema.toml`| Schema file path         |

## Implementation notes

### Language

Rust. Single binary, fast startup. DuckDB has stable Rust bindings via `duckdb-rs`.

### Dependencies

- `duckdb` (bundled) — embedded database
- `clap` — argument parsing
- `toml` — schema parsing
- `serde` / `serde_json` — serialization
- `chrono` — timestamps
- `dirs` — home directory resolution

### Concurrency

DuckDB allows multiple concurrent readers but only one writer. Since `ctx` is a short-lived CLI process (writes take under 5ms), the lock window is small. But multiple agents can collide.

Write commands (`add`, `set`, `rm`, `link`, `unlink`, `define`, `extend`) use a retry loop:

- On `SQLITE_BUSY` / lock error: retry up to 3 times
- Backoff: 50ms, 100ms, 200ms (exponential)
- Total worst-case delay: 350ms
- If all retries fail: exit with an error and a message like `ctx: database locked after 3 retries`

Read commands (`get`, `find`, `search`, `walk`, `path`, `edges`, `count`, `schema`, `kinds`, `stats`) open the database in read-only mode and never block.

This is sufficient for the expected workload: multiple agents in separate terminals, sub-agents running in parallel, or hooks firing concurrently. True write contention (two agents calling `ctx add` at the same millisecond) is uncommon, and a 50ms retry is invisible to the agent.

### Performance targets

- `ctx add` < 5ms
- `ctx find` (indexed) < 10ms at 1M nodes
- `ctx walk` (3 hops) < 50ms at 1M edges
- `ctx search` (FTS) < 100ms at 1M nodes

### Later

- Session ingest from `~/.claude` and `~/.codex` directories
- MCP server interface so any agent framework can use it
- Graph visualization export (DOT/Mermaid)
- Remote sync between machines
