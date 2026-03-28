---
name: setup
description: Install or upgrade the ctx CLI and plugin. Handles building from source, installing the binary, registering the plugin marketplace, and verifying everything works. Run this once on a new machine or after pulling updates.
allowed-tools: Bash(cargo *), Bash(ctx *), Bash(claude *), Bash(git *), Bash(which *), Bash(ls *), Bash(cat *)
---

# ctx setup

Install or upgrade ctx. Run through each step, skip what's already done.

## 1. Check if ctx repo exists

Look for the ctx repo. If not cloned:
```bash
git clone https://github.com/michaelangeloio/ctx.git ~/developer/ctx
```

If it exists, pull latest:
```bash
git -C ~/developer/ctx pull --ff-only
```

## 2. Build and install the binary

```bash
cargo install --path ~/developer/ctx/crates/cli --force
```

Verify:
```bash
which ctx
ctx schema Session
```

If `ctx schema` works, the binary is good.

## 3. Install the Claude Code plugin

Check if already installed:
```bash
claude plugin list 2>/dev/null | grep "ctx@ctx"
```

If not installed:
```bash
claude plugin marketplace add ~/developer/ctx 2>/dev/null || true
claude plugin install ctx@ctx --scope user
```

If already installed, update it:
```bash
claude plugin update ctx@ctx 2>/dev/null || true
```

## 4. Verify the database

```bash
ctx stats
```

If this is a fresh install, the database at `~/.ctx/ctx.db` is created automatically on first use.

## 5. Show the schema

Print the full schema so the user can see what's available:
```bash
ctx schema
```

## Done

Report what was done:
- Whether ctx was freshly installed or upgraded
- Whether the plugin was freshly installed or updated
- Current node/edge counts from `ctx stats`

$ARGUMENTS
