#!/bin/bash
set -e

# Cap heap to prevent OOM kills (Claude Code runs on bun/node)
export NODE_OPTIONS="${NODE_OPTIONS:---max-old-space-size=384}"

# Run setup script if bind-mounted (sourced so PATH exports persist)
if [ -f /workspace/setup.sh ]; then
    source /workspace/setup.sh
fi

# Read all stdin into a private temp file
INPUT_FILE=$(mktemp /tmp/borg-input.XXXXXX)
chmod 600 "$INPUT_FILE"

cat > "$INPUT_FILE"

# Parse input JSON — write to a temp vars file and source it (avoids eval injection)
VARS_FILE=$(mktemp /tmp/borg-vars.XXXXXX)
chmod 600 "$VARS_FILE"

INPUT_FILE="$INPUT_FILE" bun -e "
const d=JSON.parse(require('fs').readFileSync(process.env.INPUT_FILE,'utf8'));
const esc = s => s.replace(/'/g, \"'\\\\''\");
process.stdout.write('PROMPT=\'' + esc(d.prompt||'') + \"'\\n\");
process.stdout.write('MODEL=\'' + esc(d.model||'claude-sonnet-4-6') + \"'\\n\");
process.stdout.write('SESSION_ID=\'' + esc(d.resumeSessionId||d.sessionId||'') + \"'\\n\");
process.stdout.write('ASSISTANT_NAME=\'' + esc(d.assistantName||'Borg') + \"'\\n\");
process.stdout.write('SYSTEM_PROMPT=\'' + esc(d.systemPrompt||'') + \"'\\n\");
process.stdout.write('ALLOWED_TOOLS=\'' + esc(d.allowedTools||'') + \"'\\n\");
process.stdout.write('WORKDIR=\'' + esc(d.workdir||'') + \"'\\n\");
" > "$VARS_FILE" || { echo "Failed to parse input JSON" >&2; exit 1; }
# shellcheck source=/dev/null
source "$VARS_FILE"

# Change to workdir if specified (must be under /workspace)
if [ -n "$WORKDIR" ]; then
    case "$WORKDIR" in
        /workspace|/workspace/*)
            if [ -d "$WORKDIR" ]; then
                cd "$WORKDIR"
            else
                echo "Warning: WORKDIR $WORKDIR does not exist, staying in $(pwd)" >&2
            fi
            ;;
        *)
            echo "Warning: WORKDIR $WORKDIR is not under /workspace, ignoring" >&2
            ;;
    esac
fi

# Build claude args
CLAUDE_ARGS=(
    --print
    --output-format stream-json
    --model "$MODEL"
    --verbose
)

if [ -n "$SESSION_ID" ]; then
    CLAUDE_ARGS+=(--resume "$SESSION_ID")
fi

# Use specified allowed tools, or default to full set
if [ -n "$ALLOWED_TOOLS" ]; then
    CLAUDE_ARGS+=(--allowedTools "$ALLOWED_TOOLS")
else
    CLAUDE_ARGS+=(
        --allowedTools 'Bash,Read,Write,Edit,Glob,Grep,WebSearch,WebFetch,Task,TaskOutput,TaskStop,NotebookEdit,EnterPlanMode,ExitPlanMode,TaskCreate,TaskGet,TaskUpdate,TaskList'
    )
fi

CLAUDE_ARGS+=(--permission-mode bypassPermissions)

# Prepend system prompt to user prompt if provided
if [ -n "$SYSTEM_PROMPT" ]; then
    FULL_PROMPT="$SYSTEM_PROMPT

---

$PROMPT"
else
    FULL_PROMPT="$PROMPT"
fi

# Run Claude Code — capture output to a temp file so we can check if it's empty
CLAUDE_OUT=$(mktemp /tmp/borg-claude-out.XXXXXX)
STDERR_FILE=$(mktemp /tmp/borg-stderr.XXXXXX)
trap 'rm -f "$INPUT_FILE" "$VARS_FILE" "$CLAUDE_OUT" "$STDERR_FILE"' EXIT

exitcode=0
printf '%s\n' "$FULL_PROMPT" | claude "${CLAUDE_ARGS[@]}" >"$CLAUDE_OUT" 2>"$STDERR_FILE" || exitcode=$?

# Stream output to stdout
cat "$CLAUDE_OUT"

# If no output was produced, emit an error so the pipeline can see what went wrong
if [ ! -s "$CLAUDE_OUT" ] && [ -s "$STDERR_FILE" ]; then
    echo '{"type":"error","message":"Claude CLI produced no output. Stderr:"}'
    cat "$STDERR_FILE" >&2
fi

exit "$exitcode"
