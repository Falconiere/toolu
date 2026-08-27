#!/usr/bin/env bats

setup() {
  TMP=$(mktemp -d)
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -m init -q
}

teardown() {
  rm -rf "$TMP"
}

source_lib() {
  # shellcheck disable=SC1091
  . "${BATS_TEST_DIRNAME}/../detect.sh"
}

@test "detect_project_root returns git toplevel" {
  source_lib
  run detect_project_root
  [ "$status" -eq 0 ]
  [ -n "$output" ]
}

@test "detect_project_name returns basename" {
  source_lib
  run detect_project_name
  [ "$status" -eq 0 ]
  [ "$output" = "$(basename "$(pwd -P)")" ]
}

@test "detect_project_name returns 0 outside a git repo under set -e (fallback survives)" {
  # A bare `[ -n "$root" ] && basename` exits 1 here; under set -e that aborts
  # a caller before its own fallback runs. The helper must exit 0 and print "".
  run bash -c 'set -euo pipefail; . "'"${BATS_TEST_DIRNAME}"'/../detect.sh"; cd /tmp
    P="${X:-$(detect_project_name)}"; [ -z "$P" ] && P="unknown"; echo "name=[$P]"'
  [ "$status" -eq 0 ]
  [ "$output" = "name=[unknown]" ]
}

@test "detect_node_pm returns bun when bun.lock present" {
  touch bun.lock
  source_lib
  run detect_node_pm
  [ "$output" = "bun" ]
}

@test "detect_node_pm returns pnpm when pnpm-lock.yaml present" {
  touch pnpm-lock.yaml
  source_lib
  run detect_node_pm
  [ "$output" = "pnpm" ]
}

@test "detect_node_pm returns npm when package-lock.json present" {
  touch package-lock.json
  source_lib
  run detect_node_pm
  [ "$output" = "npm" ]
}

@test "detect_node_pm returns empty when no lock file" {
  source_lib
  run detect_node_pm
  [ -z "$output" ]
}

@test "detect_rust returns rust when Cargo.toml present" {
  touch Cargo.toml
  source_lib
  run detect_rust
  [ "$output" = "rust" ]
}

@test "detect_rust returns empty when no Cargo.toml" {
  source_lib
  run detect_rust
  [ -z "$output" ]
}

@test "detect_ts returns ts when tsconfig.json present" {
  echo '{}' > tsconfig.json
  git add tsconfig.json
  git -c user.email=t@t -c user.name=t commit -q -m tsconfig
  source_lib
  run detect_ts
  [ "$output" = "ts" ]
}

@test "detect_ts_linter: biome wins over oxc and eslint" {
  touch biome.json .oxlintrc.json .eslintrc.json
  source_lib
  run detect_ts_linter
  [ "$output" = "biome" ]
}

@test "detect_ts_linter: oxc when only .oxlintrc.json" {
  touch .oxlintrc.json
  source_lib
  run detect_ts_linter
  [ "$output" = "oxc" ]
}

@test "detect_ts_linter: eslint for legacy .eslintrc.cjs" {
  touch .eslintrc.cjs
  source_lib
  run detect_ts_linter
  [ "$output" = "eslint" ]
}

@test "detect_ts_linter: empty when no linter config" {
  source_lib
  run detect_ts_linter
  [ -z "$output" ]
}

@test "count_code_lines: excludes blanks and // comments" {
  printf '%s\n' 'let a = 1;' '' '// a comment' '   ' 'let b = 2;' > f.ts
  source_lib
  run count_code_lines f.ts
  [ "$output" = "2" ]
}

@test "count_code_lines: excludes multi-line /* */ block" {
  printf '%s\n' 'let a = 1;' '/*' ' block' ' comment' '*/' 'let b = 2;' > f.rs
  source_lib
  run count_code_lines f.rs
  [ "$output" = "2" ]
}

@test "count_code_lines: code with trailing comment still counts" {
  printf '%s\n' 'let a = 1; // trailing' '// pure comment' 'let b = 2;' > f.ts
  source_lib
  run count_code_lines f.ts
  [ "$output" = "2" ]
}

@test "count_code_lines: inline /* */ leaving code counts; rust /// dropped" {
  printf '%s\n' 'let a = /* x */ 1;' '/// doc line' 'let b = 2;' > f.rs
  source_lib
  run count_code_lines f.rs
  [ "$output" = "2" ]
}

@test "detect_clippy: clippy token when config present" {
  touch clippy.toml
  source_lib
  run detect_clippy
  [ "$output" = "clippy" ]
}

@test "detect_clippy: empty when absent" {
  source_lib
  run detect_clippy
  [ -z "$output" ]
}

@test "detect_base_branch falls back to main when no remote" {
  source_lib
  run detect_base_branch
  [ "$output" = "main" ]
}

@test "detect_project_root returns empty outside git" {
  cd /tmp
  source_lib
  run detect_project_root
  [ -z "$output" ]
}

@test "to_relative_path strips git toplevel prefix from absolute path" {
  source_lib
  root=$(detect_project_root)
  run to_relative_path "$root/hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  [ "$output" = "hooks/lib/detect.sh" ]
}

@test "to_relative_path passes through a relative path unchanged" {
  source_lib
  run to_relative_path "hooks/lib/detect.sh"
  [ "$status" -eq 0 ]
  [ "$output" = "hooks/lib/detect.sh" ]
}

@test "to_relative_path passes through an abs path outside the repo unchanged" {
  source_lib
  run to_relative_path "/etc/passwd"
  [ "$status" -eq 0 ]
  [ "$output" = "/etc/passwd" ]
}

@test "to_relative_path on empty input returns empty" {
  source_lib
  run to_relative_path ""
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "strip_heredocs: <<EOF > /tmp/x body is stripped, trailing command preserved" {
  source_lib
  out=$(printf '%s\n' "cat <<EOF > /tmp/x" "body1" "body2" "EOF" "echo after" | strip_heredocs)
  [[ "$out" == *"cat <<EOF > /tmp/x"* ]]
  [[ "$out" == *"echo after"* ]]
  [[ "$out" != *"body1"* ]]
  [[ "$out" != *"body2"* ]]
}

@test "strip_heredocs: <<-END (tab-indented form) is stripped" {
  source_lib
  out=$(printf '%s\n' "cat <<-END" $'\tbody1' $'\tbody2' $'\tEND' "echo end" | strip_heredocs)
  [[ "$out" == *"cat <<-END"* ]]
  [[ "$out" == *"echo end"* ]]
  [[ "$out" != *"body1"* ]]
}

@test "strip_heredocs: <<DOC alternate delimiter is stripped" {
  source_lib
  out=$(printf '%s\n' "cat <<DOC" "x" "y" "DOC" "echo end" | strip_heredocs)
  [[ "$out" == *"echo end"* ]]
  [[ "$out" != *"^x$"* ]]
  printf '%s\n' "$out" | grep -qxF "x" && return 1
  return 0
}

@test "strip_heredocs: plain command (no heredoc) passes through unchanged" {
  source_lib
  out=$(printf '%s\n' "echo hello" "ls -la" | strip_heredocs)
  [ "$out" = "$(printf '%s\n' "echo hello" "ls -la")" ]
}

@test "strip_heredocs: <<EOF | tee (pipe after heredoc start) strips body" {
  source_lib
  out=$(printf '%s\n' "cat <<EOF | tee /tmp/x" "secret cargo test inside body" "EOF" "echo done" | strip_heredocs)
  [[ "$out" != *"secret cargo test inside body"* ]]
  [[ "$out" == *"echo done"* ]]
}

@test "read_list: missing file returns no output" {
  source_lib
  run read_list "$BATS_TEST_TMPDIR/nope.txt"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "read_list: skips comments and blank lines" {
  source_lib
  f="$BATS_TEST_TMPDIR/list.txt"
  printf '%s\n' "# comment" "" "real-line" "  # indented comment" "another" > "$f"
  run read_list "$f"
  [ "$status" -eq 0 ]
  [ "$output" = "$(printf '%s\n' 'real-line' 'another')" ]
}

@test "detect_plugin_installed: echoes spec when registry contains the key" {
  source_lib
  reg="$BATS_TEST_TMPDIR/installed.json"
  printf '%s\n' '{"plugins":{"code-simplifier@claude-plugins-official":[{"scope":"user"}]}}' > "$reg"
  CLAUDE_PLUGINS_REGISTRY="$reg" run detect_plugin_installed "code-simplifier@claude-plugins-official"
  [ "$status" -eq 0 ]
  [ "$output" = "code-simplifier@claude-plugins-official" ]
}

@test "detect_plugin_installed: empty + exit 0 when key absent" {
  source_lib
  reg="$BATS_TEST_TMPDIR/installed.json"
  printf '%s\n' '{"plugins":{"other@marketplace":[]}}' > "$reg"
  CLAUDE_PLUGINS_REGISTRY="$reg" run detect_plugin_installed "code-simplifier@claude-plugins-official"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "detect_plugin_installed: tolerates non-array value (regression for length>0 bug)" {
  source_lib
  reg="$BATS_TEST_TMPDIR/installed.json"
  # Value is a number — prior `.plugins[$s] | length > 0` filter errored;
  # `has($s)` returns true (key is present, regardless of value shape).
  printf '%s\n' '{"plugins":{"caveman@caveman":42}}' > "$reg"
  CLAUDE_PLUGINS_REGISTRY="$reg" run detect_plugin_installed "caveman@caveman"
  [ "$status" -eq 0 ]
  [ "$output" = "caveman@caveman" ]
}

@test "detect_plugin_installed: indeterminate (exit 2) when registry missing" {
  source_lib
  CLAUDE_PLUGINS_REGISTRY="$BATS_TEST_TMPDIR/does-not-exist.json" \
    run detect_plugin_installed "code-simplifier@claude-plugins-official"
  [ "$status" -eq 2 ]
  [ -z "$output" ]
}

@test "detect_plugin_installed: indeterminate (exit 2) when registry malformed" {
  source_lib
  reg="$BATS_TEST_TMPDIR/installed.json"
  printf '%s\n' 'not json' > "$reg"
  CLAUDE_PLUGINS_REGISTRY="$reg" run detect_plugin_installed "code-simplifier@claude-plugins-official"
  [ "$status" -eq 2 ]
  [ -z "$output" ]
}

@test "detect_plugin_installed: indeterminate (exit 2) when plugins key wrong type" {
  source_lib
  reg="$BATS_TEST_TMPDIR/installed.json"
  printf '%s\n' '{"plugins":"not-an-object"}' > "$reg"
  CLAUDE_PLUGINS_REGISTRY="$reg" run detect_plugin_installed "code-simplifier@claude-plugins-official"
  [ "$status" -eq 2 ]
  [ -z "$output" ]
}

@test "detect_plugin_installed: empty spec returns 0 + empty" {
  source_lib
  run detect_plugin_installed ""
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "count_code_lines: unterminated /* falls back to raw line count" {
  printf '%s\n' 'let a = 1;' 'let s = "/*";' 'let b = 2;' 'let c = 3;' > f.ts
  source_lib
  run count_code_lines f.ts
  [ "$output" = "4" ]
}

# ── comemory version detection ──────────────────────────────────────────────
# Stub `comemory` on PATH so these assert the helper logic, not the host's
# installed version.
_stub_comemory() {  # $1 = version string the stub reports
  mkdir -p "$TMP/stub"
  printf '#!/bin/sh\necho "comemory %s"\n' "$1" > "$TMP/stub/comemory"
  chmod +x "$TMP/stub/comemory"
}

@test "comemory_version parses X.Y.Z from --version output" {
  source_lib
  _stub_comemory "1.2.3"
  PATH="$TMP/stub:$PATH" run comemory_version
  [ "$status" -eq 0 ]
  [ "$output" = "1.2.3" ]
}

@test "comemory_version_ok: 0 when installed > minimum" {
  source_lib
  _stub_comemory "0.9.0"
  PATH="$TMP/stub:$PATH" run comemory_version_ok
  [ "$status" -eq 0 ]
}

@test "comemory_version_ok: 0 when installed == minimum (boundary)" {
  source_lib
  _stub_comemory "$COMEMORY_MIN_VERSION"
  PATH="$TMP/stub:$PATH" run comemory_version_ok
  [ "$status" -eq 0 ]
}

@test "comemory_version_ok: 1 when installed < minimum" {
  source_lib
  _stub_comemory "0.6.0"
  PATH="$TMP/stub:$PATH" run comemory_version_ok
  [ "$status" -eq 1 ]
}

@test "comemory_version picks comemory's own version, not a trailing dependency version" {
  source_lib
  mkdir -p "$TMP/stub"
  printf '#!/bin/sh\necho "comemory 1.2.3 (built against sqlite 3.45.0)"\n' > "$TMP/stub/comemory"
  chmod +x "$TMP/stub/comemory"
  PATH="$TMP/stub:$PATH" run comemory_version
  [ "$status" -eq 0 ]
  [ "$output" = "1.2.3" ]
}

@test "comemory_version_ok: 2 (indeterminate) when comemory absent" {
  source_lib
  bin="$TMP/cleanbin"; mkdir -p "$bin"
  for t in bash sh grep sort head; do ln -s "$(command -v "$t")" "$bin/$t" 2>/dev/null || true; done
  PATH="$bin" run comemory_version_ok
  [ "$status" -eq 2 ]
}

# ── branch_slug ─────────────────────────────────────────────────────────────
# Shared with push-review.sh / write-state.sh: '/'→'_', strip to safe charset,
# empty → "_default". Keyed branch-slug naming for transient state files.

@test "branch_slug: feat/foo -> feat_foo" {
  source_lib
  run branch_slug "feat/foo"
  [ "$status" -eq 0 ]
  [ "$output" = "feat_foo" ]
}

@test "branch_slug: a/b/c -> a_b_c (all slashes)" {
  source_lib
  run branch_slug "a/b/c"
  [ "$status" -eq 0 ]
  [ "$output" = "a_b_c" ]
}

@test "branch_slug: weird chars stripped (feat#\$% -> feat)" {
  source_lib
  run branch_slug 'feat#$%'
  [ "$status" -eq 0 ]
  [ "$output" = "feat" ]
}

@test "branch_slug: empty -> _default" {
  source_lib
  run branch_slug ""
  [ "$status" -eq 0 ]
  [ "$output" = "_default" ]
}

# ── is_git_push ─────────────────────────────────────────────────────────────
# Shared push detection: strip_heredocs then the exact push-review regex.
# Returns 0 iff the command is a real `git push` (boundary-anchored).

@test "is_git_push: bare 'git push' matches" {
  source_lib
  run is_git_push "git push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'git push origin main' matches" {
  source_lib
  run is_git_push "git push origin main"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'x && git push' matches" {
  source_lib
  run is_git_push "x && git push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'foo; git push' matches" {
  source_lib
  run is_git_push "foo; git push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'gitpush' does NOT match" {
  source_lib
  run is_git_push "gitpush"
  [ "$status" -ne 0 ]
}

@test "is_git_push: 'git status' does NOT match" {
  source_lib
  run is_git_push "git status"
  [ "$status" -ne 0 ]
}

@test "is_git_push: 'git pushup' does NOT match (boundary required after push)" {
  source_lib
  run is_git_push "git pushup"
  [ "$status" -ne 0 ]
}

@test "is_git_push: 'git push' inside heredoc body is ignored" {
  source_lib
  run is_git_push "$(printf '%s\n' 'cat <<EOF' 'about git push' 'EOF')"
  [ "$status" -ne 0 ]
}

@test "is_git_push: 'git -C <path> push' matches (pr-babysit's worktree form)" {
  source_lib
  run is_git_push "git -C /tmp/wt push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'git -C \"<quoted path>\" push origin HEAD' matches" {
  source_lib
  run is_git_push 'git -C "/tmp/my wt" push origin HEAD'
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'git -c k=v push' matches" {
  source_lib
  run is_git_push "git -c push.default=simple push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'git --no-pager push' matches" {
  source_lib
  run is_git_push "git --no-pager push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'git --git-dir=/x/.git push' matches" {
  source_lib
  run is_git_push "git --git-dir=/x/.git push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'git -C /tmp/wt status' does NOT match" {
  source_lib
  run is_git_push "git -C /tmp/wt status"
  [ "$status" -ne 0 ]
}

@test "is_git_push: 'git commit -m \"push\"' does NOT match (subcommand is not an option)" {
  source_lib
  run is_git_push 'git commit -m "push"'
  [ "$status" -ne 0 ]
}

@test "push_target_root: 'git -C <worktree> push' resolves the worktree root" {
  source_lib
  git -c user.email=t@t -c user.name=t checkout -q -b feature
  git -c user.email=t@t -c user.name=t commit --allow-empty -qm work
  git checkout -q -
  git worktree add -q "$TMP/wt" feature
  run push_target_root "git -C $TMP/wt push"
  [ "$status" -eq 0 ]
  [ "$output" = "$(cd "$TMP/wt" && pwd -P)" ]
}

@test "push_target_root: bare 'git push' resolves the cwd's root" {
  source_lib
  run push_target_root "git push"
  [ "$status" -eq 0 ]
  [ "$output" = "$(pwd -P)" ]
}

@test "push_target_root: unresolvable -C path falls back to the cwd's root" {
  source_lib
  run push_target_root "git -C /nonexistent/nope push"
  [ "$status" -eq 0 ]
  [ "$output" = "$(pwd -P)" ]
}

@test "push_target_root: outside git uses the Codex project override" {
  source_lib
  outside="$BATS_TEST_TMPDIR/outside"
  project="$BATS_TEST_TMPDIR/codex-project"
  mkdir -p "$outside" "$project"
  cd "$outside"
  TOOLU_HOST_OVERRIDE=codex TOOLU_PROJECT_DIR="$project" run push_target_root "git push"
  [ "$status" -eq 0 ]
  [ "$output" = "$project" ]
}

@test "push_target_root: -C from a different command in the chain is ignored" {
  source_lib
  git -c user.email=t@t -c user.name=t checkout -q -b feature
  git -c user.email=t@t -c user.name=t commit --allow-empty -qm work
  git checkout -q -
  git worktree add -q "$TMP/wt" feature
  other=$(mktemp -d)
  git -C "$other" init -q
  # The `-C $other` belongs to the status call, not the push.
  run push_target_root "git -C $other status && git -C $TMP/wt push"
  rm -rf "$other"
  [ "$output" = "$(cd "$TMP/wt" && pwd -P)" ]
}

@test "push_target_root: cumulative -C flags compose like git's own" {
  source_lib
  mkdir -p "$TMP/outer/inner"
  git -C "$TMP/outer/inner" init -q
  # git applies each -C relative to the previous one.
  run push_target_root "git -C $TMP/outer -C inner push"
  [ "$output" = "$(cd "$TMP/outer/inner" && pwd -P)" ]
}

# ── is_git_commit (AC-26) ───────────────────────────────────────────────────

@test "is_git_commit matches a plain commit" {
  source_lib
  run is_git_commit 'git commit -m "feat: x"'
  [ "$status" -eq 0 ]
}

@test "is_git_commit matches git -C <path> commit" {
  source_lib
  run is_git_commit 'git -C /tmp/worktree commit -m "fix: y"'
  [ "$status" -eq 0 ]
}

@test "is_git_commit matches a global-option form" {
  source_lib
  run is_git_commit 'git --no-pager commit'
  [ "$status" -eq 0 ]
}

@test "is_git_commit matches a commit after a chain operator" {
  source_lib
  run is_git_commit 'git add -A && git commit -m "chore: z"'
  [ "$status" -eq 0 ]
}

@test "is_git_commit does not match git commit-tree" {
  source_lib
  run is_git_commit 'git commit-tree abc123'
  [ "$status" -ne 0 ]
}

@test "is_git_commit does not match a bare word" {
  source_lib
  run is_git_commit 'gitcommit'
  [ "$status" -ne 0 ]
}

@test "is_git_commit does not match commit prose inside a heredoc body" {
  source_lib
  run is_git_commit 'cat <<EOF > notes.txt
remember to git commit later
EOF'
  [ "$status" -ne 0 ]
}

@test "is_git_commit does not match a push" {
  source_lib
  run is_git_commit 'git push origin feat/x'
  [ "$status" -ne 0 ]
}

# ── prose is not a command (the echo false positive) ────────────────────────
#
# Detection used to be a regex over the raw string, so any text containing the
# two words in sequence fired the push gates — including a shell command whose
# only crime was talking about pushing.

@test "is_git_push: the words inside an echo argument are not a push" {
  source_lib
  run is_git_push 'echo "no git push rules remain"'
  [ "$status" -ne 0 ]
}

@test "is_git_push: the words inside a single-quoted argument are not a push" {
  source_lib
  run is_git_push "echo 'git push'"
  [ "$status" -ne 0 ]
}

@test "is_git_push: a grep pattern mentioning the subcommand is not a push" {
  source_lib
  run is_git_push 'grep -qE "^git push" plugins/toolu/settings/bash-denylist.txt'
  [ "$status" -ne 0 ]
}

@test "is_git_push: a commit message mentioning it is not a push" {
  source_lib
  run is_git_push 'git commit -m "explain why git push is gated"'
  [ "$status" -ne 0 ]
}

@test "is_git_commit: the same message IS a commit" {
  source_lib
  run is_git_commit 'git commit -m "explain why git push is gated"'
  [ "$status" -eq 0 ]
}

@test "is_git_push: an operator inside a quoted argument does not split a statement" {
  source_lib
  run is_git_push 'echo "first; git push"'
  [ "$status" -ne 0 ]
}

@test "is_git_commit: prose about committing is not a commit" {
  source_lib
  run is_git_commit 'echo "remember to git commit later"'
  [ "$status" -ne 0 ]
}

# ── but a real invocation still counts, however it is reached ───────────────

@test "is_git_push: a push inside command substitution matches" {
  source_lib
  run is_git_push 'out=$(git push origin HEAD)'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a push inside backticks matches" {
  source_lib
  run is_git_push 'out=`git push`'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a push inside a substitution within double quotes matches" {
  source_lib
  run is_git_push 'echo "$(git push)"'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a real push after a quoted argument containing an operator matches" {
  source_lib
  run is_git_push 'echo "a; b" && git push'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a quoted subcommand still matches" {
  source_lib
  run is_git_push 'git "push"'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a pipeline ending in a push matches" {
  source_lib
  run is_git_push 'echo hi | git push'
  [ "$status" -eq 0 ]
}

@test "is_git_push: newline-separated statements are seen" {
  source_lib
  run is_git_push "$(printf '%s\n' 'echo one' 'git push')"
  [ "$status" -eq 0 ]
}

@test "is_git_push: an empty command is not a push" {
  source_lib
  run is_git_push ""
  [ "$status" -ne 0 ]
}

@test "is_git_push: a path that merely contains the words is not a push" {
  source_lib
  run is_git_push 'cat /tmp/git push notes.txt'
  [ "$status" -ne 0 ]
}

# ── parsing corners that hid a real push ───────────────────────────────────
#
# Both of these were caught in review: the detector said "no push" about a
# command line that pushes, which is the dangerous direction to be wrong in.

@test "is_git_push: a quoted paren inside a substitution does not truncate the scan" {
  source_lib
  run is_git_push 'echo "$(echo ")")" ; git push'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a single-quoted paren inside a substitution is handled too" {
  source_lib
  run is_git_push "echo \"\$(echo ')')\" && git push"
  [ "$status" -eq 0 ]
}

@test "is_git_push: a nested substitution still resolves" {
  source_lib
  run is_git_push 'echo "$(echo "$(git push)")"'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a push in a case arm is seen" {
  source_lib
  run is_git_push 'case x in a) git push;; esac'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a push in a subshell is seen" {
  source_lib
  run is_git_push '(cd /tmp/wt && git push)'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a push in a function body is seen" {
  source_lib
  run is_git_push 'deploy() { git push; }'
  [ "$status" -eq 0 ]
}

@test "is_git_commit: a commit in a case arm is seen" {
  source_lib
  run is_git_commit 'case $x in ready) git commit -m "feat: x";; esac'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a parenthesised word in prose is still not a push" {
  source_lib
  run is_git_push 'echo "run it (git push) later"'
  [ "$status" -ne 0 ]
}

@test "is_git_push: statements with spaces and globs are not word-split" {
  source_lib
  run is_git_push 'git push origin "my branch"'
  [ "$status" -eq 0 ]
  run is_git_push 'git push *.txt'
  [ "$status" -eq 0 ]
}

@test "is_git_push: git with a dangling -C and no subcommand is not a push" {
  source_lib
  run is_git_push 'git -C'
  [ "$status" -ne 0 ]
}

@test "is_git_push: an environment assignment before the command is seen" {
  source_lib
  run is_git_push 'GIT_SSH_COMMAND="ssh -i k" git push'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a negated push is seen" {
  source_lib
  run is_git_push 'if ! git push; then echo failed; fi'
  [ "$status" -eq 0 ]
}

@test "is_git_push: 'echo git push' is still not a push" {
  source_lib
  run is_git_push 'echo git push'
  [ "$status" -ne 0 ]
}

@test "is_git_push: a removed substitution does not fuse text into a command" {
  source_lib
  # Neither side is a command; joining them would manufacture `git push`.
  run is_git_push 'gi$(echo t) push'
  [ "$status" -ne 0 ]
}

@test "is_git_push: a removed backtick substitution does not fuse either" {
  source_lib
  run is_git_push 'gi`echo t` push'
  [ "$status" -ne 0 ]
}

@test "is_git_push: text either side of a substitution stays separate" {
  source_lib
  run is_git_push 'echo "pre $(echo x) post" && git push'
  [ "$status" -eq 0 ]
}

@test "is_git_push: a dangling -c with no value is not a push" {
  source_lib
  run is_git_push 'git -c'
  [ "$status" -ne 0 ]
}

@test "is_git_push: a dangling -C before a real push in the next statement still matches" {
  source_lib
  run is_git_push 'git -C; git push'
  [ "$status" -eq 0 ]
}
