#!/usr/bin/env bash
set -euo pipefail

# Read session JSON from stdin (provided by Claude Code on SessionStart)
read -r INPUT

SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "unknown"')
MODEL=$(echo "$INPUT" | jq -r '.model // "unknown"')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path // "unknown"')
CWD=$(echo "$INPUT" | jq -r '.cwd // "unknown"')
BRANCH=$(git -C "$CWD" branch --show-current 2>/dev/null || echo "")

# Only register if ctx is installed and session_id is valid
if ! command -v ctx &>/dev/null; then
  exit 0
fi

if [ "$SESSION_ID" = "unknown" ]; then
  exit 0
fi

# Check if session already registered (idempotent)
if ctx find Session "session_id=$SESSION_ID" --quiet 2>/dev/null | grep -q .; then
  exit 0
fi

# Register session
ARGS=(
  "session_id=$SESSION_ID"
  "title=Session $SESSION_ID"
  "tool=claude"
  "model=$MODEL"
  "project_path=$CWD"
)

[ -n "$BRANCH" ] && ARGS+=("git_branch=$BRANCH")
[ "$TRANSCRIPT" != "unknown" ] && ARGS+=("filepath=$TRANSCRIPT")

ctx add Session "${ARGS[@]}" >/dev/null 2>&1 || true

# Inject session context so the agent knows its session ID
cat <<EOF
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "ctx session registered. Session ID: $SESSION_ID | Model: $MODEL | Transcript: $TRANSCRIPT\n\nUse ctx to record highlights, topics, and artifacts during this session. Run ctx schema <Kind> for property hints."
  }
}
EOF
