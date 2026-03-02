#!/bin/bash
# Tests for git commit event emission in entrypoint.sh.
# Exercises the commit block in isolation using a real temp git repo.
set -euo pipefail

PASS=0
FAIL=0

pass() { echo "PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "FAIL: $1"; FAIL=$((FAIL + 1)); }

# Emit events to stderr as the entrypoint does, captured in $EVENTS_OUT.
EVENTS_OUT=$(mktemp)
trap 'rm -f "$EVENTS_OUT"' EXIT

log_event() {
    echo "---BORG_EVENT---${1}" >> "$EVENTS_OUT"
}

run_commit_block() {
    local repo_dir="$1"
    local commit_msg="$2"
    local git_cmd="${3:-git}"  # allows injecting a fake git

    EVENTS_OUT_LOCAL=$(mktemp)

    (
        cd "$repo_dir"
        log_event_local() { echo "---BORG_EVENT---${1}" >> "$EVENTS_OUT_LOCAL"; }

        if ! git diff --quiet HEAD 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]; then
            git add -A
            if $git_cmd commit -m "$commit_msg"; then
                log_event_local "{\"type\":\"container_event\",\"event\":\"commit_complete\",\"message\":\"${commit_msg}\"}"
            else
                log_event_local "{\"type\":\"container_event\",\"event\":\"commit_failed\",\"message\":\"${commit_msg}\"}"
            fi
        else
            log_event_local "{\"type\":\"container_event\",\"event\":\"commit_skipped\"}"
        fi
    )

    cat "$EVENTS_OUT_LOCAL"
    rm -f "$EVENTS_OUT_LOCAL"
}

# ── helpers ──────────────────────────────────────────────────────────────────

make_repo() {
    local d
    d=$(mktemp -d)
    git -C "$d" init -q
    git -C "$d" config user.name "Test"
    git -C "$d" config user.email "test@test.com"
    # initial commit so HEAD exists
    echo "init" > "$d/init.txt"
    git -C "$d" add init.txt
    git -C "$d" commit -q -m "init"
    echo "$d"
}

events_contain() {
    local events="$1"
    local pattern="$2"
    echo "$events" | grep -q "$pattern"
}

# ── Test 1: successful commit emits commit_complete ──────────────────────────

T1_REPO=$(make_repo)
echo "new file" > "$T1_REPO/foo.txt"

T1_EVENTS=$(run_commit_block "$T1_REPO" "feat: add foo")

if events_contain "$T1_EVENTS" '"event":"commit_complete"'; then
    pass "successful commit emits commit_complete"
else
    fail "successful commit emits commit_complete (got: $T1_EVENTS)"
fi

if ! events_contain "$T1_EVENTS" '"event":"commit_failed"'; then
    pass "successful commit does NOT emit commit_failed"
else
    fail "successful commit must not emit commit_failed"
fi

rm -rf "$T1_REPO"

# ── Test 2: failed commit emits commit_failed, not commit_complete ───────────

T2_REPO=$(make_repo)
echo "new file" > "$T2_REPO/bar.txt"

# Fake git that succeeds for everything except 'commit'
FAKE_GIT=$(mktemp)
chmod +x "$FAKE_GIT"
cat > "$FAKE_GIT" <<'EOF'
#!/bin/bash
if [ "${1:-}" = "commit" ]; then
    echo "error: simulated commit failure" >&2
    exit 1
fi
exec git "$@"
EOF

T2_EVENTS=$(run_commit_block "$T2_REPO" "feat: bar" "$FAKE_GIT")

if events_contain "$T2_EVENTS" '"event":"commit_failed"'; then
    pass "failed commit emits commit_failed"
else
    fail "failed commit emits commit_failed (got: $T2_EVENTS)"
fi

if ! events_contain "$T2_EVENTS" '"event":"commit_complete"'; then
    pass "failed commit does NOT emit commit_complete"
else
    fail "failed commit must not emit commit_complete"
fi

rm -f "$FAKE_GIT"
rm -rf "$T2_REPO"

# ── Test 3: no changes emits commit_skipped ──────────────────────────────────

T3_REPO=$(make_repo)
# No new files — working tree is clean

T3_EVENTS=$(run_commit_block "$T3_REPO" "feat: nothing")

if events_contain "$T3_EVENTS" '"event":"commit_skipped"'; then
    pass "clean working tree emits commit_skipped"
else
    fail "clean working tree emits commit_skipped (got: $T3_EVENTS)"
fi

rm -rf "$T3_REPO"

# ── Test 4: commit message is preserved in commit_failed event ───────────────

T4_REPO=$(make_repo)
echo "file" > "$T4_REPO/baz.txt"

FAKE_GIT2=$(mktemp)
chmod +x "$FAKE_GIT2"
cat > "$FAKE_GIT2" <<'EOF'
#!/bin/bash
if [ "${1:-}" = "commit" ]; then exit 1; fi
exec git "$@"
EOF

T4_EVENTS=$(run_commit_block "$T4_REPO" "fix: my specific message" "$FAKE_GIT2")

if events_contain "$T4_EVENTS" 'fix: my specific message'; then
    pass "commit message preserved in commit_failed event"
else
    fail "commit message preserved in commit_failed event (got: $T4_EVENTS)"
fi

rm -f "$FAKE_GIT2"
rm -rf "$T4_REPO"

# ── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
