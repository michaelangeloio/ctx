---
name: setup
description: Set up ctx for Codex. Builds the binary, configures session auto-registration, installs skills, and adds agent instructions. Run once on a new machine.
allowed-tools: Bash(cargo *), Bash(ctx *), Bash(git *), Bash(which *), Bash(cat *), Bash(chmod *), Bash(grep *), Bash(cp *), Bash(mkdir *), Read, Write
---

# ctx setup for Codex

Walk through each step. Skip anything already done. Ask before modifying config files.

## 1. Find or clone the repo

```bash
ls ~/ctx/Cargo.toml 2>/dev/null || ls ~/developer/ctx/Cargo.toml 2>/dev/null
```

If not found:
```bash
git clone https://github.com/michaelangeloio/ctx.git ~/ctx
```

If found, pull latest:
```bash
git -C <path> pull --ff-only
```

## 2. Build the binary

```bash
cargo install --path <path>/crates/cli --force
which ctx && ctx stats
```

## 3. Configure session auto-registration

Read `~/.codex/session-info.sh`. If it already contains `ctx add Session`, skip this step.

If the file exists but has no ctx block, append after the existing session ID extraction:

```bash
# Register in ctx if available
if command -v ctx &>/dev/null && [ "$SESSION_ID" != "unknown" ]; then
  if ! ctx find Session "session_id=$SESSION_ID" --quiet 2>/dev/null | grep -q .; then
    BRANCH=$(git -C "$CWD" branch --show-current 2>/dev/null || echo "")
    ARGS=("session_id=$SESSION_ID" "title=Session $SESSION_ID" "tool=codex" "model=$MODEL" "project_path=$CWD")
    [ -n "$BRANCH" ] && ARGS+=("git_branch=$BRANCH")
    [ "$TRANSCRIPT" != "unknown" ] && ARGS+=("filepath=$TRANSCRIPT")
    ctx add Session "${ARGS[@]}" >/dev/null 2>&1 || true
  fi
fi
```

If `~/.codex/hooks.json` doesn't have a SessionStart hook referencing `session-info.sh`, create or update it:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "~/.codex/session-info.sh"
          }
        ]
      }
    ]
  }
}
```

## 4. Install skills

```bash
cp -r <path>/plugin/skills/ingest-session ~/.codex/skills/ctx-ingest-session
cp -r <path>/plugin/skills/graph-context ~/.codex/skills/ctx-graph-context
```

## 5. Add agent instructions to ~/.codex/instructions.md

Check if already present:
```bash
grep -c "ctx — agent memory" ~/.codex/instructions.md 2>/dev/null
```

If not found, append to `~/.codex/instructions.md`:

```
## ctx — agent memory

Use the `ctx` CLI to record what you work on and query prior context. The database is at `~/.ctx/ctx.db`.

Before starting work, check the graph for prior context.

At the start of a session:
\```
ctx add Session...
\```

As you work, record key moments, artifacts, and anything else in the schema:
\```
ctx add Highlight...
ctx add Artifact...
ctx link...
\```

Run `ctx schema <Kind>` to see all properties and hints for any node type.
```

## 6. Verify

```bash
ctx schema
ctx stats
```

Tell the user to restart Codex to pick up the new skills.

$ARGUMENTS
