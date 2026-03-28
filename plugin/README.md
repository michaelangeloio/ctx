# ctx plugin

Connects AI coding agents to the ctx context graph. Auto-registers sessions and provides skills for ingesting transcripts and querying prior context.

Works with Claude Code and Codex.

## What it does

- **Auto-registration**: Creates a Session node when a session starts
- **`/ctx:ingest-session`**: Reads a session transcript and populates the graph
- **`/ctx:graph-context`**: Queries prior work relevant to the current task

## Install

Prerequisite: `ctx` binary on your PATH.

```bash
git clone https://github.com/michaelangeloio/ctx.git
cd ctx
cargo install --path crates/cli
```

### Claude Code

```bash
claude plugin marketplace add .
claude plugin install ctx@ctx --scope user
```

The plugin includes a `SessionStart` hook that auto-registers sessions.

### Codex

Codex plugins don't support hooks, so session registration is done via the existing `~/.codex/session-info.sh` hook. Add this to your `~/.codex/session-info.sh`:

```bash
# Register in ctx if available
if command -v ctx &>/dev/null && [ "$SESSION_ID" != "unknown" ]; then
  if ! ctx find Session "session_id=$SESSION_ID" --quiet 2>/dev/null | grep -q .; then
    ctx add Session session_id=$SESSION_ID title="Session $SESSION_ID" \
      tool=codex model=$MODEL project_path=$CWD >/dev/null 2>&1 || true
  fi
fi
```

For skills, copy the plugin to your Codex plugins directory:

```bash
cp -r plugin ~/.codex/plugins/ctx
```

## Development

Test without installing:

```bash
claude --plugin-dir ./plugin
```

Update after pulling changes:

```bash
claude plugin update ctx@ctx
```
