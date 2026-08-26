#!/usr/bin/env bats
# Tests for hooks/session-start.sh — must remain project-agnostic.

HOOK="${BATS_TEST_DIRNAME}/../session-start.sh"

setup() {
  TMP=$(mktemp -d)
}

teardown() {
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

@test "session-start: Codex snapshots installed plugins into its native cache" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  mkdir -p "$TMP/bin" "$TMP/codex"
  cat > "$TMP/bin/codex" <<'SH'
#!/usr/bin/env bash
printf '%s\n' '{"installed":[{"pluginId":"toolu@toolu","name":"toolu","marketplaceName":"toolu","installed":true}],"available":[]}'
SH
  chmod +x "$TMP/bin/codex"
  run env PATH="$TMP/bin:$PATH" TOOLU_HOST_OVERRIDE=codex CODEX_HOME="$TMP/codex" \
    bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  [ "$(jq -r .status "$TMP/codex/toolu/codex-plugins.json")" = ready ]
  [ "$(jq -r '.plugins[0]' "$TMP/codex/toolu/codex-plugins.json")" = toolu@toolu ]
}

@test "session-start: runs without error in an empty git repo" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
}

@test "session-start: does not print project-specific yamless/routo literals" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -qE 'yamless|routo|/Volumes/Projects/(routo|yamless)'
}

@test "session-start: runs without error outside any git repo" {
  cd /tmp
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
}

@test "session-start: orphan sweep removes the legacy statusline symlink" {
  cd "$TMP"
  mkdir -p "$TMP/cfg/toolu"
  ln -s "$TMP/cfg/toolu/gone.sh" "$TMP/cfg/toolu/statusline.sh"
  run env CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  [ ! -L "$TMP/cfg/toolu/statusline.sh" ]
  [ ! -e "$TMP/cfg/toolu/statusline.sh" ]
}

@test "session-start: orphan sweep never deletes a real statusline file" {
  cd "$TMP"
  mkdir -p "$TMP/cfg/toolu"
  printf 'user-owned' > "$TMP/cfg/toolu/statusline.sh"
  run env CLAUDE_CONFIG_DIR="$TMP/cfg" bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  [ -f "$TMP/cfg/toolu/statusline.sh" ]
  [ "$(cat "$TMP/cfg/toolu/statusline.sh")" = "user-owned" ]
}

@test "session-start: emits per-toolchain doc only when toolchain detected" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  # No tsconfig, no Cargo.toml — neither block should appear.
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'TypeScript notes'
  ! echo "$output" | grep -q 'Rust notes'
}

@test "session-start: toolchain block stays off by default even when detected" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  # Track a tsconfig so detect_ts returns "ts", but omit TOOLU_VERBOSE.
  echo '{}' > tsconfig.json
  git -c user.email=t@t -c user.name=t add tsconfig.json
  git -c user.email=t@t -c user.name=t commit -q -m ts
  unset TOOLU_VERBOSE
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'TypeScript notes'
}

@test "session-start: TOOLU_VERBOSE=0 keeps toolchain block off" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  echo '{}' > tsconfig.json
  git -c user.email=t@t -c user.name=t add tsconfig.json
  git -c user.email=t@t -c user.name=t commit -q -m ts
  export TOOLU_VERBOSE=0
  run bash "$HOOK" <<<'{"source":"startup"}'
  unset TOOLU_VERBOSE
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q 'TypeScript notes'
}

@test "session-start: source=compact triggers compact branch with post-compaction doc" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  run bash "$HOOK" <<<'{"source":"compact"}'
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.systemMessage')" = "Context compacted" ]
  echo "$output" | jq -r '.hookSpecificOutput.additionalContext' | grep -q 'Recover memories'
}

@test "session-start: source=resume triggers resume branch" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  run bash "$HOOK" <<<'{"source":"resume"}'
  [ "$status" -eq 0 ]
  [ "$(echo "$output" | jq -r '.systemMessage')" = "Session resumed" ]
}

@test "session-start: project name with sed metacharacters renders safely" {
  mkdir -p "$TMP/we|ird&name"
  cd "$TMP/we|ird&name"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -re '.hookSpecificOutput.additionalContext')
  # Exact substitution: name appears verbatim in the rendered doc header,
  # without literal surrounding quotes and without leftover tokens.
  echo "$ctx" | grep -qF 'Session Protocol — we|ird&name'
  ! echo "$ctx" | grep -qF '"we|ird&name"'
  ! echo "$ctx" | grep -qF '{{project_name}}'
}

@test "session-start: rendering is exact under stock /bin/bash (3.2 on macOS)" {
  mkdir -p "$TMP/we|ird&name"
  cd "$TMP/we|ird&name"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  run /bin/bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -re '.hookSpecificOutput.additionalContext')
  echo "$ctx" | grep -qF 'Session Protocol — we|ird&name'
  ! echo "$ctx" | grep -qF '"we|ird&name"'
  ! echo "$ctx" | grep -qF '{{project_name}}'
}

@test "session-start: git context is branch-only (no volatile dirty count)" {
  # Cache discipline: the SessionStart additionalContext lands in the cached
  # prefix, so it must depend only on stable inputs. A live uncommitted-file
  # count goes stale immediately and differs every session — keep branch only.
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  echo dirty > file.txt   # working tree is now dirty
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -re '.hookSpecificOutput.additionalContext')
  branch=$(git -C "$TMP" symbolic-ref --short HEAD)
  echo "$ctx" | grep -qF "Branch: $branch"
  ! echo "$ctx" | grep -qiE 'uncommitted|[0-9]+ files'
}

@test "session-start: failing quality gate is not injected into the cached prefix" {
  # The failing gate is surfaced live (statusline) and per-prompt
  # (user-prompt-submit). Re-injecting it into the once-cached SessionStart
  # prefix is redundant and volatile, so it must not appear here.
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  mkdir -p "$TMP/.claude/tmp"
  printf '%s' '{"status":"failing","reason":"boom"}' > "$TMP/.claude/tmp/quality-gate-status.json"
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  ctx=$(echo "$output" | jq -re '.hookSpecificOutput.additionalContext')
  ! echo "$ctx" | grep -qiE 'quality gate failing|boom'
}

@test "session-start: toolchain block emits when TOOLU_VERBOSE=1 and detected" {
  cd "$TMP"
  git init -q
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  echo '{}' > tsconfig.json
  git -c user.email=t@t -c user.name=t add tsconfig.json
  git -c user.email=t@t -c user.name=t commit -q -m ts
  export TOOLU_VERBOSE=1
  run bash "$HOOK" <<<'{"source":"startup"}'
  unset TOOLU_VERBOSE
  [ "$status" -eq 0 ]
  echo "$output" | grep -q 'TypeScript notes'
}

# ── Housekeeping wiring (AC-18) ─────────────────────────────────────────────

# A repo with isolated config/home so the notice sentinel and the permission
# write land inside the sandbox rather than on the machine running the suite.
housekeeping_repo() {
  cd "$TMP"
  git init -q -b main
  git -c user.email=t@t -c user.name=t commit --allow-empty -q -m init
  mkdir -p "$TMP/home/.claude" "$TMP/.claude/tmp/push-review"
  export HOME="$TMP/home"
  export CLAUDE_CONFIG_DIR="$TMP/home/.claude"
  export CLAUDE_PROJECT_DIR="$TMP"
  export TOOLU_PROJECT_DIR="$TMP"
}

@test "session-start: announces the new gate default once" {
  housekeeping_repo
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  [[ "$output" == *"balanced"* ]]
  [[ "$output" == *"preset"* ]]
  [ -f "$TMP/home/.claude/toolu/.gate-preset-notice-v5" ]

  run bash "$HOOK" <<<'{"source":"startup"}'
  [[ "$output" != *"gates now default"* ]]
}

@test "session-start: stays quiet about the default when gates are configured" {
  housekeeping_repo
  printf '%s' '{"version":1,"gates":{"preset":"strict"}}' > "$TMP/.claude/toolu.config.json"
  run bash "$HOOK" <<<'{"source":"startup"}'
  [[ "$output" != *"gates now default"* ]]
}

@test "session-start: writes the permission allowlist once and says so" {
  housekeeping_repo
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ -f "$TMP/.claude/settings.local.json" ]
  [ "$(jq -r '.permissions.allow | index("Bash(*)") != null' "$TMP/.claude/settings.local.json")" = "true" ]
  [[ "$output" == *"settings.local.json"* ]]

  run bash "$HOOK" <<<'{"source":"startup"}'
  [[ "$output" != *"one time only"* ]]
}

@test "session-start: sweeps state for a branch that no longer exists" {
  housekeeping_repo
  printf '{"version":2}' > "$TMP/.claude/tmp/push-review/gone_branch.json"
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ "$status" -eq 0 ]
  [ ! -f "$TMP/.claude/tmp/push-review/gone_branch.json" ]
}

@test "session-start: leaves the current branch's state alone" {
  housekeeping_repo
  git checkout -qb feat/live
  echo x > x.txt && git add x.txt && git -c user.email=t@t -c user.name=t commit -qm x
  printf '{"version":2}' > "$TMP/.claude/tmp/push-review/feat_live.json"
  run bash "$HOOK" <<<'{"source":"startup"}'
  [ -f "$TMP/.claude/tmp/push-review/feat_live.json" ]
}

@test "session-start: still starts when the state dir cannot be read" {
  housekeeping_repo
  printf '{"version":2}' > "$TMP/.claude/tmp/push-review/gone_branch.json"
  chmod 000 "$TMP/.claude/tmp/push-review"
  run bash "$HOOK" <<<'{"source":"startup"}'
  chmod 755 "$TMP/.claude/tmp/push-review"
  [ "$status" -eq 0 ]
  [ -n "$output" ]
}
