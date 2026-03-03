#!/bin/bash
# Tests that stderr_tail escaping in entrypoint.sh produces valid JSON.
set -euo pipefail

pass=0
fail=0

check() {
    local desc="$1"
    local input="$2"
    local json_value
    json_value=$(printf '%s' "$input" | bun -e "let s='';process.stdin.on('data',c=>s+=c);process.stdin.on('end',()=>process.stdout.write(JSON.stringify(s)));")
    local full_json="{\"stderr_tail\":${json_value}}"
    if printf '%s' "$full_json" | bun -e "JSON.parse(require('fs').readFileSync('/dev/stdin','utf8'))" >/dev/null 2>&1; then
        echo "PASS: $desc"
        pass=$(( pass + 1 ))
    else
        echo "FAIL: $desc"
        echo "  input:  $(printf '%s' "$input" | cat -v)"
        echo "  output: $full_json"
        fail=$(( fail + 1 ))
    fi
}

check "plain text"           "hello world"
check "double quotes"        'say "hello"'
check "backslash"            'path\to\file'
check "newlines"             $'line one\nline two\nline three'
check "carriage return"      $'foo\rbar'
check "tab"                  $'col1\tcol2'
check "backslash-n literal"  'error: \n not a real newline'
check "null byte surrogate"  "$(printf 'before\x01after')"
check "mixed specials"       $'"quotes"\\ and\nnewlines\twith\r\nwindows'
check "unicode"              "café résumé naïve"
check "empty string"         ""
check "only newlines"        $'\n\n\n'

echo ""
echo "Results: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
