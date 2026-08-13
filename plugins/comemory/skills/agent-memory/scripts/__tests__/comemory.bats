#!/usr/bin/env bats
# Round-trip tests for the comemory wrapper module against the REAL comemory
# binary (no mocks). Uses a unique throwaway --repo label so saved memories
# never collide with real project memories, and deletes everything it creates
# in teardown.

# `run --separate-stderr` (used by the miss-banner tests) is a 1.5.0+ flag;
# declare the floor so bats does not warn (BW02) about flag use.
bats_require_minimum_version 1.5.0

COMEMORY_SH="${BATS_TEST_DIRNAME}/../comemory.sh"

# Throwaway repo: the wrapper auto-detects the repo from the git toplevel, so
# we override it to a label nothing else uses.
TEST_REPO="toolu-mig-test"
export MY_CLAUDE_COMEMORY_REPO="$TEST_REPO"

setup() {
  command -v comemory >/dev/null 2>&1 || skip "comemory binary not installed"
}

teardown() {
  # Soft-delete every memory created under the throwaway repo. Tolerant of an
  # empty list and of a missing binary (setup skips, but teardown still runs).
  command -v comemory >/dev/null 2>&1 || return 0
  command -v jq >/dev/null 2>&1 || return 0
  # 0.9.0's `list --json` is a {items,...} envelope; 0.8.x was a bare array.
  # Handle both shapes when harvesting ids to clean up.
  local id
  for id in $(comemory list --repo "$TEST_REPO" --json 2>/dev/null \
      | jq -r 'if type=="array" then .[] else .items[] end | .id' 2>/dev/null); do
    [ -n "$id" ] && comemory delete "$id" --json >/dev/null 2>&1 || true
  done
}

@test "comemory: save then search round-trips against the real binary" {
  # Save through the wrapper; --json (pass-through flag) emits {"id":...,"path":...}.
  run bash "$COMEMORY_SH" save "mig-test-title" "mig-test real body" --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ]
  [ "$id" != "null" ]

  # The saved memory's id must surface in the wrapper's search results.
  run bash "$COMEMORY_SH" search "mig-test-title"
  [ "$status" -eq 0 ]
  [[ "$output" == *"$id"* ]]
}

@test "comemory: summary saves a session-summary and accepts a --kind override without flag collision" {
  # summary must not hardcode --kind, so a caller-supplied --kind reaches
  # comemory cleanly (clap rejects a duplicate single-value flag).
  run bash "$COMEMORY_SH" summary "wrapped up the migration" --kind decision --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ]
  [ "$id" != "null" ]
  # The override landed: kind is the caller's value, not a forced default.
  run bash -c "comemory list --repo '$TEST_REPO' --json | jq -r '(if type==\"array\" then .[] else .items[] end) | select(.id==\"$id\") | .kind'"
  [ "$status" -eq 0 ]
  [ "$output" = "decision" ]
}

@test "comemory: summary yields to a caller-supplied --tags without flag collision" {
  # A non-zero exit here is the collision signature: had the wrapper ALSO
  # injected its own --tags session-summary, comemory/clap would reject the
  # duplicate single-value flag and the save would fail. status 0 proves the
  # wrapper suppressed its default tag in favour of the caller's.
  run bash "$COMEMORY_SH" summary "custom tagged summary" --tags "release,notes" --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ]
  [ "$id" != "null" ]
  # The memory really persisted (the save was not silently partial).
  run bash "$COMEMORY_SH" list --json
  [ "$status" -eq 0 ]
  [[ "$output" == *"$id"* ]]
}

# A stub `comemory` on PATH echoes its argv one-per-line, so these assert the
# EXACT argv the wrapper builds (behavioral) rather than grepping wrapper source.
_stub_argv() {
  STUB="$BATS_TEST_TMPDIR/argv-stub"
  mkdir -p "$STUB"
  printf '#!/bin/sh\nfor a in "$@"; do printf "%%s\\n" "$a"; done\n' > "$STUB/comemory"
  chmod +x "$STUB/comemory"
}

@test "comemory: wrapper injects --repo <repo> and guards the positional with -- (behavioral argv)" {
  _stub_argv
  run env PATH="$STUB:$PATH" MY_CLAUDE_COMEMORY_REPO=behave bash "$COMEMORY_SH" search "hello"
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | grep -cx -- '--repo')" -eq 1 ]
  printf '%s\n' "$output" | grep -qx 'behave'
  printf '%s\n' "$output" | grep -qx -- '--'      # end-of-options guard present
  printf '%s\n' "$output" | grep -qx 'hello'      # query passed as positional
}

@test "comemory: host-neutral repo override takes precedence over the Claude legacy alias" {
  _stub_argv
  run env PATH="$STUB:$PATH" TOOLU_COMEMORY_REPO=neutral \
    MY_CLAUDE_COMEMORY_REPO=legacy bash "$COMEMORY_SH" search "hello"
  [ "$status" -eq 0 ]
  printf '%s\n' "$output" | grep -qx 'neutral'
  ! printf '%s\n' "$output" | grep -qx 'legacy'
}

@test "comemory: caller --repo suppresses the wrapper's injection — no duplicate (behavioral argv)" {
  _stub_argv
  run env PATH="$STUB:$PATH" MY_CLAUDE_COMEMORY_REPO=behave bash "$COMEMORY_SH" search "hi" --repo caller
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | grep -cx -- '--repo')" -eq 1 ]
  printf '%s\n' "$output" | grep -qx 'caller'
  ! printf '%s\n' "$output" | grep -qx 'behave'
}

@test "comemory: search-code/index-code/graph carry --repo (behavioral argv)" {
  _stub_argv
  run env PATH="$STUB:$PATH" MY_CLAUDE_COMEMORY_REPO=behave bash "$COMEMORY_SH" search-code "sym"
  [ "$status" -eq 0 ]; printf '%s\n' "$output" | grep -qx -- '--repo'; printf '%s\n' "$output" | grep -qx 'behave'
  run env PATH="$STUB:$PATH" MY_CLAUDE_COMEMORY_REPO=behave bash "$COMEMORY_SH" graph
  [ "$status" -eq 0 ]; printf '%s\n' "$output" | grep -qx -- '--repo'
  run env PATH="$STUB:$PATH" MY_CLAUDE_COMEMORY_REPO=behave bash "$COMEMORY_SH" index-code --path /tmp/x
  [ "$status" -eq 0 ]; printf '%s\n' "$output" | grep -qx -- '--repo'
}

@test "comemory: filter_project is gone from the wrapper" {
  ! grep -q 'filter_project' "$COMEMORY_SH"
}

@test "comemory: save with a leading-dash title is not parsed as a flag (real binary)" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-dash"
  run bash "$COMEMORY_SH" save "--dashy-title" "real body" --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ] && [ "$id" != "null" ]
}

@test "comemory: search-code runs against the real binary (lexical, empty index → no results, exit 0)" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-sc"
  run bash "$COMEMORY_SH" search-code "nonexistent_symbol_xyz"
  [ "$status" -eq 0 ]
}

# ── Retrieval-loop verbs: GLOBAL (must NOT inject --repo) ───────────────────
@test "comemory: global loop verbs exec without --repo injection" {
  # The combined branch forwards the subcommand verbatim — no --repo appended.
  grep -q 'mine|tune|eval|prune|gc|rebuild)' "$COMEMORY_SH"
  grep -q 'exec comemory "\$subcmd" "\$@"' "$COMEMORY_SH"
  # feedback forwards the positional query_id after the -- guard, still no --repo.
  grep -q 'exec comemory feedback "\$@" -- "\$query_id"' "$COMEMORY_SH"
  # No --repo anywhere on the global-verb lines.
  ! grep -E 'comemory (mine|tune|eval|prune|gc|rebuild|feedback).*--repo' "$COMEMORY_SH"
}

@test "comemory: maintain runs mine+prune+gc against the real binary (isolated store, exit 0)" {
  # Isolated data dir so prune --apply / gc never touch the real store.
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-maint"
  mkdir -p "$COMEMORY_DATA_DIR"
  run bash "$COMEMORY_SH" maintain
  [ "$status" -eq 0 ]
}

@test "comemory: caller-supplied --repo overrides without a clap duplicate-flag collision" {
  # The wrapper must NOT also inject --repo when the caller passed one; a second
  # --repo would clap-collide and exit non-zero.
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-repo"
  run bash "$COMEMORY_SH" save "ovr-title" "ovr body" --repo custom-scope --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ] && [ "$id" != "null" ]
  # It landed under the caller's repo, not the auto-detected default.
  run bash -c "comemory list --repo custom-scope --json | jq -r 'if type==\"array\" then .[] else .items[] end | .id'"
  [[ "$output" == *"$id"* ]]
}

@test "comemory: flag-like MY_CLAUDE_COMEMORY_REPO falls back to unknown (no flag injection)" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-flag"
  MY_CLAUDE_COMEMORY_REPO="-evil" run bash "$COMEMORY_SH" list --json
  [ "$status" -eq 0 ]
}

# ── Worktree scope: --repo must be the REPO, not the per-worktree checkout dir ─
# Regression: detect_project_root used `git rev-parse --show-toplevel`, which in
# a worktree is the worktree dir — so saves made in a worktree got an orphan
# `--repo <worktree-name>` scope, invisible from main and sibling worktrees.
# Build a REAL repo + worktree (no mocks for git) and capture the injected
# --repo via the argv stub so the real store is untouched.
_make_repo_with_worktree() {
  REPO_DIR="$BATS_TEST_TMPDIR/myrepo"
  WT_DIR="$BATS_TEST_TMPDIR/wt-feature"
  git init -q "$REPO_DIR"
  git -C "$REPO_DIR" config user.email t@t.t
  git -C "$REPO_DIR" config user.name t
  git -C "$REPO_DIR" commit -q --allow-empty -m init
  git -C "$REPO_DIR" worktree add -q "$WT_DIR" >/dev/null 2>&1
}

@test "comemory: --repo resolves to the repo name from inside a git worktree (real worktree, argv stub)" {
  _stub_argv
  _make_repo_with_worktree
  # MY_CLAUDE_COMEMORY_REPO is exported suite-wide; unset it so auto-detection runs.
  cd "$WT_DIR"
  run env -u MY_CLAUDE_COMEMORY_REPO PATH="$STUB:$PATH" bash "$COMEMORY_SH" search "hi"
  [ "$status" -eq 0 ]
  local repo_val
  repo_val=$(printf '%s\n' "$output" | awk '/^--repo$/{getline; print; exit}')
  [ "$repo_val" = "myrepo" ]        # the repo, shared across worktrees
  [ "$repo_val" != "wt-feature" ]   # NOT the per-worktree checkout dir
}

@test "comemory: --repo matches between the main checkout and its worktree (real worktree, argv stub)" {
  _stub_argv
  _make_repo_with_worktree
  cd "$REPO_DIR"
  run env -u MY_CLAUDE_COMEMORY_REPO PATH="$STUB:$PATH" bash "$COMEMORY_SH" search "hi"
  [ "$status" -eq 0 ]
  local from_main
  from_main=$(printf '%s\n' "$output" | awk '/^--repo$/{getline; print; exit}')
  cd "$WT_DIR"
  run env -u MY_CLAUDE_COMEMORY_REPO PATH="$STUB:$PATH" bash "$COMEMORY_SH" search "hi"
  [ "$status" -eq 0 ]
  local from_wt
  from_wt=$(printf '%s\n' "$output" | awk '/^--repo$/{getline; print; exit}')
  [ "$from_main" = "myrepo" ]
  [ "$from_wt" = "$from_main" ]
}

# ── setup verb: dispatches to scripts/setup.sh, BEFORE the binary guard ───────
@test "comemory: setup verb dispatches to scripts/setup.sh (binary-independent)" {
  run bash "$COMEMORY_SH" setup -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: setup.sh"* ]]
}

@test "comemory: setup runs BEFORE the binary-presence guard (absent binary → setup's MISSING, not the wrapper no-op)" {
  # The short-circuit must fire before the `command -v comemory` guard — else an
  # absent binary would no-op setup, the one case it exists for. With the COMEMORY
  # seam pointing at nothing, setup.sh prints MISSING; the wrapper never reaches
  # its own "not installed — skipped" branch or usage().
  COMEMORY=/nonexistent/comemory run bash "$COMEMORY_SH" setup
  [ "$status" -eq 0 ]
  [ "$(printf '%s' "$output" | awk 'NR==1{print $1}')" = "MISSING" ]
  [[ "$output" != *"Usage: comemory.sh"* ]]
  [[ "$output" != *"skipped (no-op)"* ]]
}

# ── context (repo-scoped, like search) + delete (global, no --repo) ──────────
@test "comemory: context injects --repo and guards the query with -- (behavioral argv)" {
  _stub_argv
  run env PATH="$STUB:$PATH" MY_CLAUDE_COMEMORY_REPO=behave bash "$COMEMORY_SH" context "run_migration"
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | grep -cx -- '--repo')" -eq 1 ]
  printf '%s\n' "$output" | grep -qx 'behave'
  printf '%s\n' "$output" | grep -qx -- '--'             # end-of-options guard
  printf '%s\n' "$output" | grep -qx 'run_migration'     # query as positional
}

@test "comemory: delete forwards the id after -- and injects NO --repo (behavioral argv)" {
  _stub_argv
  run env PATH="$STUB:$PATH" MY_CLAUDE_COMEMORY_REPO=behave bash "$COMEMORY_SH" delete "a1b2c3d4"
  [ "$status" -eq 0 ]
  ! printf '%s\n' "$output" | grep -qx -- '--repo'       # id is global — never scoped
  printf '%s\n' "$output" | grep -qx -- '--'
  printf '%s\n' "$output" | grep -qx 'a1b2c3d4'
}

@test "comemory: save then delete removes the memory from list (real binary)" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-del"
  mkdir -p "$COMEMORY_DATA_DIR"
  run bash "$COMEMORY_SH" save "del-title" "deletable body" --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ] && [ "$id" != "null" ]
  # Present before delete.
  run bash "$COMEMORY_SH" list --json
  [[ "$output" == *"$id"* ]]
  # Delete through the wrapper, then it must be gone from list.
  run bash "$COMEMORY_SH" delete "$id"
  [ "$status" -eq 0 ]
  run bash "$COMEMORY_SH" list --json
  [ "$status" -eq 0 ]
  ! printf '%s\n' "$output" | grep -q "$id"
}

@test "comemory: context runs against the real binary and surfaces a saved memory" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-ctx"
  mkdir -p "$COMEMORY_DATA_DIR"
  run bash "$COMEMORY_SH" save "ctx-title" "distinctivecontextterm body" --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ] && [ "$id" != "null" ]
  run bash "$COMEMORY_SH" context "distinctivecontextterm"
  [ "$status" -eq 0 ]
  [[ "$output" == *"$id"* ]]
}

@test "comemory: feedback round-trips against the real binary (isolated store)" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-fb"
  mkdir -p "$COMEMORY_DATA_DIR"
  # Save a memory, then search --json to obtain the query_id the loop feeds back.
  run bash "$COMEMORY_SH" save "fb-title" "fb body content" --json
  [ "$status" -eq 0 ]
  local mem_id qid
  mem_id=$(echo "$output" | jq -r '.id')
  [ -n "$mem_id" ] && [ "$mem_id" != "null" ]
  run bash "$COMEMORY_SH" search "fb-title" --json
  [ "$status" -eq 0 ]
  qid=$(echo "$output" | jq -r '.query_id // .queryId // empty')
  if [ -z "$qid" ]; then
    skip "comemory search --json did not expose a query_id field in this version"
  fi
  run bash "$COMEMORY_SH" feedback "$qid" --used "$mem_id" --json
  [ "$status" -eq 0 ]
}

@test "comemory: search miss emits the save-back banner on stderr, never on stdout (real binary)" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-miss-plain"
  mkdir -p "$COMEMORY_DATA_DIR"
  run --separate-stderr bash "$COMEMORY_SH" search "zzqq-no-such-memory-xyz"
  [ "$status" -eq 0 ]                       # exit code preserved on a miss
  [[ "$stderr" == *"no memory hit"* ]]      # nudge reaches the agent...
  [[ "$stderr" == *"save it back"* ]]
  [[ "$output" != *"no memory hit"* ]]      # ...and never pollutes stdout
}

@test "comemory: search --json miss keeps stdout pure JSON, banner only on stderr (real binary)" {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-miss-json"
  mkdir -p "$COMEMORY_DATA_DIR"
  run --separate-stderr bash "$COMEMORY_SH" search "zzqq-no-such-memory-xyz" --json
  [ "$status" -eq 0 ]
  echo "$output" | jq -e '.total == 0' >/dev/null   # stdout still parses; zero hits
  [[ "$output" != *"no memory hit"* ]]               # no banner bytes mixed into JSON
  [[ "$stderr" == *"save it back"* ]]                # banner still delivered via stderr
}

@test "comemory: search with a real hit does NOT emit the miss banner (real binary)" {
  command -v jq >/dev/null 2>&1 || skip "jq not installed"
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-hit-no-banner"
  mkdir -p "$COMEMORY_DATA_DIR"
  run bash "$COMEMORY_SH" save "hit-probe-title" "hit probe body" --json
  [ "$status" -eq 0 ]
  local id
  id=$(echo "$output" | jq -r '.id')
  [ -n "$id" ] && [ "$id" != "null" ]
  run --separate-stderr bash "$COMEMORY_SH" search "hit-probe-title"
  [ "$status" -eq 0 ]
  [[ "$output" == *"$id"* ]]                # the hit's id is on stdout (verbatim re-emit)
  [[ "$stderr" != *"no memory hit"* ]]      # a hit must not trigger the nudge
}

@test "comemory: miss banner names the active --repo scope (real binary)" {
  export COMEMORY_DATA_DIR="$BATS_TEST_TMPDIR/cm-miss-scope"
  mkdir -p "$COMEMORY_DATA_DIR"
  run --separate-stderr bash "$COMEMORY_SH" search "zzqq-no-such-memory-xyz"
  [ "$status" -eq 0 ]
  [[ "$stderr" == *"$TEST_REPO"* ]]         # scope surfaced in the nudge (ask #4 reinforced)
}

@test "comemory: a non-zero search exit is forwarded and never treated as a miss (stubbed failure)" {
  # Shadow the real binary with a stub that fails — an ERROR (rc!=0) must not be
  # confused with an empty result, so the save-back banner must stay silent and
  # the exit code must pass through unmasked. Stubbing a crashing binary to test
  # failure handling is allowed; the data under test is still real elsewhere.
  local shimdir="$BATS_TEST_TMPDIR/shim"
  mkdir -p "$shimdir"
  cat > "$shimdir/comemory" <<'SH'
#!/usr/bin/env bash
echo "stub stdout line"
echo "boom: simulated comemory failure" >&2
exit 3
SH
  chmod +x "$shimdir/comemory"
  PATH="$shimdir:$PATH" run --separate-stderr bash "$COMEMORY_SH" search "anything"
  [ "$status" -eq 3 ]                          # exit code forwarded, not masked
  [[ "$output" == *"stub stdout line"* ]]      # comemory stdout still re-emitted
  [[ "$stderr" != *"no memory hit"* ]]         # an error is NOT a miss — no banner
}

# ── Symlinked invocation: sibling-relative paths must resolve through the link ─
# Regression: the wrapper derived its sibling paths from BASH_SOURCE, which is
# the path as INVOKED — and register.sh publishes the wrapper as a SYMLINK at
# $CLAUDE_CONFIG_DIR/comemory/comemory.sh, the path SKILL.md and the scope-hook
# deny banner both tell agents to use. Through that link
# `${BASH_SOURCE%/*}/../../../lib` pointed at a directory that does not exist,
# the repo-scope lib silently went missing, and every memory landed in the
# shared "unknown" pool while the warning blamed git. The published path is the
# ONLY path most callers use, so this broke repo scoping for every install.
_make_repo() {
  REPO_DIR="$BATS_TEST_TMPDIR/linkrepo"
  git init -q "$REPO_DIR"
  git -C "$REPO_DIR" config user.email t@t.t
  git -C "$REPO_DIR" config user.name t
  git -C "$REPO_DIR" commit -q --allow-empty -m init
}

@test "comemory: --repo still resolves when the wrapper is invoked through a symlink (real repo, argv stub)" {
  _stub_argv
  _make_repo
  # Link from a directory whose ancestors look NOTHING like the plugin tree —
  # the pre-fix path would land outside it and lose the lib.
  local linkdir="$BATS_TEST_TMPDIR/some/deep/elsewhere"
  mkdir -p "$linkdir"
  ln -sfn "$(cd "${COMEMORY_SH%/*}" && pwd)/comemory.sh" "$linkdir/comemory.sh"
  cd "$REPO_DIR"
  run env -u MY_CLAUDE_COMEMORY_REPO PATH="$STUB:$PATH" bash "$linkdir/comemory.sh" search "hi"
  [ "$status" -eq 0 ]
  local repo_val
  repo_val=$(printf '%s\n' "$output" | awk '/^--repo$/{getline; print; exit}')
  [ "$repo_val" = "linkrepo" ]   # detected through the link
  [ "$repo_val" != "unknown" ]   # NOT the shared contamination pool
}

@test "comemory: setup verb dispatches through a symlinked invocation" {
  local linkdir="$BATS_TEST_TMPDIR/link-setup"
  mkdir -p "$linkdir"
  ln -sfn "$(cd "${COMEMORY_SH%/*}" && pwd)/comemory.sh" "$linkdir/comemory.sh"
  run bash "$linkdir/comemory.sh" setup -h
  [ "$status" -eq 0 ]
  [[ "$output" == *"Usage: setup.sh"* ]]
}

# ── Empty-key warning must name the ACTUAL cause ──────────────────────────────
@test "comemory: a missing repo-scope lib warns about the lib, not about git" {
  _stub_argv
  _make_repo
  # A copy with no lib sibling — same empty key as being outside a repo, but a
  # different cause. Blaming git for a broken install misdirects the reader.
  local orphan="$BATS_TEST_TMPDIR/orphan"
  mkdir -p "$orphan"
  cp "$COMEMORY_SH" "$orphan/comemory.sh"
  cd "$REPO_DIR"
  run --separate-stderr env -u MY_CLAUDE_COMEMORY_REPO PATH="$STUB:$PATH" bash "$orphan/comemory.sh" search "hi"
  [ "$status" -eq 0 ]
  [[ "$stderr" == *"repo-scope lib not found"* ]]
  [[ "$stderr" != *"no git repo"* ]]
}

@test "comemory: outside a git repo (lib present) still warns about the missing repo" {
  _stub_argv
  local outside="$BATS_TEST_TMPDIR/not-a-repo"
  mkdir -p "$outside"
  cd "$outside"
  run --separate-stderr env -u MY_CLAUDE_COMEMORY_REPO PATH="$STUB:$PATH" bash "$COMEMORY_SH" search "hi"
  [ "$status" -eq 0 ]
  [[ "$stderr" == *"no git repo"* ]]
  [[ "$stderr" != *"repo-scope lib not found"* ]]
}

@test "comemory: --repo resolves through a multi-hop RELATIVE symlink chain (real repo, argv stub)" {
  _stub_argv
  _make_repo
  # Relative link targets are the common case (a repo-internal shim, a hand-rolled
  # `ln -s ../..` convenience link) and exercise the loop's join branch, which an
  # absolute-target link never touches. Two hops, each relative, from different
  # directories — so a join that forgot to re-base per hop lands nowhere.
  local a="$BATS_TEST_TMPDIR/hop-a" b="$BATS_TEST_TMPDIR/hop-b"
  mkdir -p "$a" "$b"
  # PHYSICAL paths on both ends: on macOS BATS_TEST_TMPDIR sits under /var, a
  # symlink to /private/var, so a relpath computed from the lexical path is off
  # by one `..` and the link dangles.
  local real b_phys
  real="$(cd "${COMEMORY_SH%/*}" && pwd -P)/comemory.sh"
  b_phys="$(cd "$b" && pwd -P)"
  ln -sfn "$(python3 -c 'import os,sys;print(os.path.relpath(sys.argv[1],sys.argv[2]))' "$real" "$b_phys")" "$b/comemory.sh"
  ln -sfn "../hop-b/comemory.sh" "$a/comemory.sh"
  [ -L "$a/comemory.sh" ] && [ -L "$b/comemory.sh" ]
  cd "$REPO_DIR"
  run env -u MY_CLAUDE_COMEMORY_REPO PATH="$STUB:$PATH" bash "$a/comemory.sh" search "hi"
  [ "$status" -eq 0 ]
  local repo_val
  repo_val=$(printf '%s\n' "$output" | awk '/^--repo$/{getline; print; exit}')
  [ "$repo_val" = "linkrepo" ]
  [ "$repo_val" != "unknown" ]
}

@test "comemory: a symlink chain past the hop cap stops instead of spinning" {
  _stub_argv
  # Not reachable via BASH_SOURCE[0] (the kernel resolves the chain to exec the
  # file, so a real over-long chain fails before the script runs) — drive the
  # guard directly against the same loop body over a synthetic 50-link chain to
  # prove it terminates and reports rather than hanging.
  local d="$BATS_TEST_TMPDIR/chain"
  mkdir -p "$d"
  : > "$d/link0"
  local i
  for i in $(seq 1 50); do ln -sfn "link$((i - 1))" "$d/link$i"; done
  run bash -c '
    _self="$1"
    case "$_self" in */*) ;; *) _self="./$_self" ;; esac
    _hops=0
    while [ -L "$_self" ]; do
      if [ "$_hops" -ge 40 ]; then
        printf "capped at %s\n" "$_self" >&2
        break
      fi
      _hops=$((_hops + 1))
      _link=$(readlink "$_self")
      case "$_link" in
        /*) _self="$_link" ;;
        *)  _self="${_self%/*}/$_link" ;;
      esac
    done
    printf "hops=%s\n" "$_hops"
  ' _ "$d/link50"
  [ "$status" -eq 0 ]
  [[ "$output" == *"hops=40"* ]]   # stopped at the cap, did not spin
}
