#!/usr/bin/env bats
# Tests for the toolu statusline. Real JSON payloads on stdin, no mocks.

SL="${BATS_TEST_DIRNAME}/../statusline.sh"

setup() {
  TMP=$(mktemp -d)
}

teardown() {
  cd /tmp
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

# Strip ANSI colour codes so assertions match plain text.
_plain() { printf '%s' "$1" | sed $'s/\033\\[[0-9;]*m//g'; }

@test "statusline: renders model and context segments" {
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":45000,"used_percentage":22}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"Opus"* ]]
  [[ "$plain" == *"ctx:45k/200k (22%)"* ]]
}

@test "statusline: effort shown when present" {
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"effort":{"level":"high"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"effort:high"* ]]
}

@test "statusline: effort omitted when absent" {
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" != *"effort:"* ]]
}

@test "statusline: red gate marker when the project gate is failing" {
  mkdir -p "$TMP/.claude/tmp"
  printf '%s' '{"status":"failing","reason":"x"}' > "$TMP/.claude/tmp/quality-gate-status.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"gate:failing"* ]]
}

@test "statusline: no gate marker when the gate is passing" {
  mkdir -p "$TMP/.claude/tmp"
  printf '%s' '{"status":"passing"}' > "$TMP/.claude/tmp/quality-gate-status.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" != *"gate:failing"* ]]
}

@test "statusline: no gate marker when there is no gate file" {
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" != *"gate:failing"* ]]
}

@test "statusline: branch + folder shown for a git workspace" {
  ( cd "$TMP" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init )
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"$(basename "$TMP")"* ]]
  # default branch name (main or master) appears
  [[ "$plain" == *"$(git -C "$TMP" symbolic-ref --short HEAD)"* ]]
}

@test "statusline: dirty shows staged files with [+N]" {
  (cd "$TMP" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init)
  touch "$TMP/staged.txt" && (cd "$TMP" && git add staged.txt)
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out") && [[ "$plain" == *"[+1]"* ]]
}

@test "statusline: dirty shows unstaged files with [~N]" {
  (cd "$TMP" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init)
  echo "c" > "$TMP/f.txt" && (cd "$TMP" && git add f.txt && git -c user.email=t@t -c user.name=t commit -qm add)
  echo "ch" >> "$TMP/f.txt"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out") && [[ "$plain" == *"[~1]"* ]]
}

@test "statusline: dirty shows untracked files with [?N]" {
  (cd "$TMP" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init)
  touch "$TMP/u.txt"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out") && [[ "$plain" == *"[?1]"* ]]
}

@test "statusline: dirty shows all three counts when mixed" {
  (cd "$TMP" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init)
  # Commit one file first so subsequent modifications show as unstaged.
  echo "c" > "$TMP/f.txt" && (cd "$TMP" && git add f.txt && git -c user.email=t@t -c user.name=t commit -qm add)
  echo "ch" >> "$TMP/f.txt"                                        # unstaged ~
  touch "$TMP/s.txt" && (cd "$TMP" && git add s.txt)               # staged +
  touch "$TMP/u.txt"                                                # untracked ?
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out") && [[ "$plain" == *"[+1 ~1 ?1]"* ]]
}

@test "statusline: clean repo omits dirty bracket" {
  (cd "$TMP" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init)
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" != *"[+"* ]] && [[ "$plain" != *"[~"* ]] && [[ "$plain" != *"[?"* ]]
}

@test "statusline: ahead arrow shown when local is ahead of upstream" {
  repo="$TMP/repo"; remote="$TMP/remote"; mkdir -p "$repo" "$remote"
  (cd "$remote" && git init -q --bare)
  (cd "$repo" && git init -q -b main && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init && git remote add origin "$remote" && git push -q -u origin main && git -c user.email=t@t -c user.name=t commit --allow-empty -qm ahead)
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$repo"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out") && [[ "$plain" == *"↑1"* ]]
}

@test "statusline: behind arrow shown when local is behind upstream" {
  repo="$TMP/repo"; remote="$TMP/remote"; mkdir -p "$repo" "$remote"
  (cd "$remote" && git init -q --bare)
  (cd "$repo" && git init -q -b main && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init && git remote add origin "$remote" && git push -q -u origin main && git -c user.email=t@t -c user.name=t commit --allow-empty -qm behind && git push -q origin main && git reset --hard HEAD~1 -q)
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$repo"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out") && [[ "$plain" == *"↓1"* ]]
}

@test "statusline: dirty and ahead shown together" {
  repo="$TMP/repo"; remote="$TMP/remote"; mkdir -p "$repo" "$remote"
  (cd "$remote" && git init -q --bare)
  (cd "$repo" && git init -q -b main && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init && git remote add origin "$remote" && git push -q -u origin main && git -c user.email=t@t -c user.name=t commit --allow-empty -qm ahead)
  touch "$repo/new.txt"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$repo"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"↑1"* ]] && [[ "$plain" == *"[?1]"* ]]
}

@test "statusline: no dirty status outside a git repo" {
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" != *"[+"* ]] && [[ "$plain" != *"[~"* ]] && [[ "$plain" != *"[?"* ]]
}

@test "statusline: gate marker resolves via git root when cwd is a subdir" {
  ( cd "$TMP" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init )
  mkdir -p "$TMP/.claude/tmp" "$TMP/packages/app/src"
  printf '%s' '{"status":"failing","reason":"x"}' > "$TMP/.claude/tmp/quality-gate-status.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'/packages/app/src"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"gate:failing"* ]]
}

@test "statusline: format_tokens M-tier renders millions with one decimal" {
  # format_tokens drives the ctx segment; feed a millions-scale token count.
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":13779513,"used_percentage":99}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"ctx:13.7M/200k"* ]]
}

@test "statusline: format_tokens M-tier stays k below a million" {
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":13779,"used_percentage":7}}' | bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"ctx:13k/200k"* ]]
}

@test "statusline: wk: renders the summed current-week usage dir" {
  wk=$(date +%G-W%V)
  mkdir -p "$TMP/cfg/statusline/usage/$wk"
  printf '{"tokens":1000000}' > "$TMP/cfg/statusline/usage/$wk/s1.json"
  printf '{"tokens":234567}'  > "$TMP/cfg/statusline/usage/$wk/s2.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"wk:1.2M"* ]]
}

@test "statusline: wk: omitted when there is no usage for the week" {
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" != *"wk:"* ]]
}

@test "statusline: wk: a non-numeric usage file does not zero the whole week" {
  wk=$(date +%G-W%V)
  mkdir -p "$TMP/cfg/statusline/usage/$wk"
  printf '{"tokens":1000000}' > "$TMP/cfg/statusline/usage/$wk/good.json"
  printf '{"tokens":"abc"}'   > "$TMP/cfg/statusline/usage/$wk/bad.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"wk:1.0M"* ]]
}

@test "statusline: comemory:renders the count from the comemory marker" {
  ( cd "$TMP" && git init -q )
  key=$(basename "$TMP")
  mkdir -p "$TMP/cfg/comemory-status"
  printf '{"repo":"%s","count":7}' "$key" > "$TMP/cfg/comemory-status/$key.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"[COMEMORY:7]"* ]]
}

@test "statusline: comemory:worktree resolves to the main-repo key" {
  main="$TMP/main"; mkdir -p "$main"
  ( cd "$main" && git init -q && git -c user.email=t@t -c user.name=t commit --allow-empty -qm init )
  git -C "$main" worktree add -q "$TMP/wt" >/dev/null 2>&1
  key=$(basename "$main")
  mkdir -p "$TMP/cfg/comemory-status"
  printf '{"repo":"%s","count":5}' "$key" > "$TMP/cfg/comemory-status/$key.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'/wt"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"[COMEMORY:5]"* ]]
}

@test "statusline: comemory:omitted when there is no marker" {
  ( cd "$TMP" && git init -q )
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$TMP"'"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" != *"[COMEMORY:"* ]]
}

# --- token-ledger: weighted cost + cache_read ---------------------------------
LEDGER="${BATS_TEST_DIRNAME}/../hooks/token-ledger.sh"

@test "token-ledger: records weighted cost + cache_read buckets from real fixtures" {
  # sub-session.jsonl auto-picks up its …/sub-session/subagents/agent-*.jsonl.
  # Reference values computed independently from the fixtures (real data, no mocks):
  #   tokens 268219, input 7554, output 17448, cache_read 4550314, cache_write 243217, cost ≈ $1.45.
  fix="${BATS_TEST_DIRNAME}/fixtures/sub-session.jsonl"
  printf '%s' '{"transcript_path":"'"$fix"'","session_id":"testsess"}' \
    | CLAUDE_CONFIG_DIR="$TMP/cfg" TZ=UTC bash "$LEDGER"
  f="$TMP/cfg/statusline/usage/2026-W24/testsess.json"
  [ -f "$f" ]
  [ "$(jq -r '.tokens'      "$f")" = "268219" ]
  [ "$(jq -r '.input'       "$f")" = "7554" ]
  [ "$(jq -r '.output'      "$f")" = "17448" ]
  [ "$(jq -r '.cache_read'  "$f")" = "4550314" ]
  [ "$(jq -r '.cache_write' "$f")" = "243217" ]
  [ "$(jq -r '.cost*100|floor' "$f")" = "145" ]
}

@test "statusline: wk: shows weekly cost and cache-hit%" {
  wk=$(date +%G-W%V)
  mkdir -p "$TMP/cfg/statusline/usage/$wk"
  printf '{"tokens":268219,"input":7554,"output":17448,"cache_read":4550314,"cache_write":243217,"cost":1.4508}' \
    > "$TMP/cfg/statusline/usage/$wk/s1.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"wk:268k"* ]]
  [[ "$plain" == *'$1.45'* ]]
  [[ "$plain" == *"cache:99%"* ]]
}

@test "statusline: wk: cost/cache omitted for legacy token-only files" {
  wk=$(date +%G-W%V)
  mkdir -p "$TMP/cfg/statusline/usage/$wk"
  printf '{"tokens":1000000}' > "$TMP/cfg/statusline/usage/$wk/s1.json"
  out=$(printf '%s' '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}' | CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$SL")
  plain=$(_plain "$out")
  [[ "$plain" == *"wk:1.0M"* ]]
  [[ "$plain" != *'$'* ]]
  [[ "$plain" != *"cache:"* ]]
}
