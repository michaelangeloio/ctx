---
name: ingest-session
description: Ingest an AI coding session transcript (JSONL) into the ctx context graph. Extracts metadata, topics, highlights, artifacts, and relationships, then creates/updates nodes and edges. Use when the user provides a session file path or session ID to ingest.
allowed-tools: Read, Bash(ctx *), Bash(jq *), Bash(head *), Agent
---

# Ingest session

Parse a session transcript and build the corresponding graph.

## Input

The user provides one of:
- A file path (`.jsonl`)
- A session ID (UUID) — resolve via `~/.claude/projects/*/SESSION_ID.jsonl` or `~/.codex/sessions/**/*SESSION_ID*.jsonl`
- A keyword — use `ctx search` to find candidates

## Step 1 — Pre-flight

```bash
ctx find Session "session_id^=SESSION_ID_PREFIX"
```

If found, enrich it (add highlights, update summary). If not, create it.

## Step 2 — Extract metadata

Session JSONL has one JSON object per line. Relevant line types:

- `type: "user"` — has `sessionId`, `gitBranch`, `cwd`, `message.content`
- `type: "assistant"` — has `message.model`, `message.content` (text and tool_use)
- `type: "system"` / `type: "file-history-snapshot"` — skip these

Extract from the first user message: `sessionId`, `gitBranch`, `cwd`, `message.content` (first prompt).
Extract from assistant messages: `message.model`.

For large files, use an Agent to read in chunks and extract: title, summary, detail, topics, highlights, artifacts.

## Step 3 — Resolve existing nodes

Before creating, check what already exists:

```bash
ctx find Project "path=THE_CWD"
ctx find Branch "name=THE_BRANCH"
ctx find Topic "name=THE_TOPIC"
```

Reuse existing nodes. Only create when no match exists.

## Step 4 — Create nodes and edges

```bash
# Session
ctx add Session session_id=UUID title="Short title" summary="1-2 sentences" \
  detail="Narrative" tool=claude model=opus project_path=/path git_branch=branch \
  filepath=/path/to/file.jsonl first_prompt="First message"

# Topics (reuse existing)
ctx add Topic name=topic-name description="What this means"
ctx link Session:ID HAS_TOPIC Topic:ID

# Highlights
ctx add Highlight content="One sentence" kind=discovery detail="Full context"
ctx link Session:ID HAS_HIGHLIGHT Highlight:ID

# Artifacts
ctx add Artifact name="groups_finder.rb" kind=file path=app/finders/groups_finder.rb
ctx link Session:ID PRODUCED Artifact:ID

# Cross-references
ctx link Session:ID CONTINUES Session:OTHER reason="Why"
ctx link Highlight:ID REFERENCES Session:OTHER context="What aspect"
ctx link Session:ID IN_PROJECT Project:ID
ctx link Session:ID ON_BRANCH Branch:ID
```

## Highlight extraction

When reading a transcript, map signals to highlight kinds:

| Signal | Kind |
|---|---|
| "I found that...", surprising measurement, root cause | discovery |
| "Let's go with...", explicit choice between options | decision |
| Dead end, irreducible constraint, "can't because..." | blocker |
| "This means...", pattern recognition, useful connection | insight |
| "We should...", "TODO", future work identified | todo |

Prefer fewer high-quality highlights. A short session might have 2-3. A deep research session might have 8-15.

## Step 5 — Verify

```bash
ctx get Session:ID
```

$ARGUMENTS
