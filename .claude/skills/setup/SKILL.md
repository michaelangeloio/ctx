---
name: setup
description: Set up ctx for Claude Code. Builds the binary, installs the plugin, configures session auto-registration, and adds agent instructions. Run once on a new machine.
allowed-tools: Bash(cargo *), Bash(ctx *), Bash(claude *), Bash(git *), Bash(which *), Bash(cat *), Bash(chmod *), Bash(grep *), Read, Write, Edit
---

# ctx setup for Claude Code

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

## 3. Install the Claude Code plugin

```bash
claude plugin marketplace add <path>
claude plugin install ctx@ctx --scope user
```

If already installed:
```bash
claude plugin update ctx@ctx
```

The plugin includes a SessionStart hook that auto-registers sessions and injects the session ID into context. If the user has an existing SessionStart hook in `~/.claude/settings.json` that runs `session-info.sh`, ask if they want to keep it or remove it — the plugin handles everything the old hook did plus ctx registration.

## 4. Add agent instructions to ~/.claude/CLAUDE.md

Check if already present:
```bash
grep -c "ctx — agent memory" ~/.claude/CLAUDE.md 2>/dev/null
```

If not found, append to `~/.claude/CLAUDE.md`:

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

## 5. Verify

```bash
ctx schema
ctx stats
```

Tell the user to restart Claude Code to pick up the plugin.

$ARGUMENTS
