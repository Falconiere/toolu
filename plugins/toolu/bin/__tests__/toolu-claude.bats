#!/usr/bin/env bats
# Tests for bin/toolu-claude — the terminal dashboard launcher. Verifies symlink
# self-resolution and exact `bun run` argument forwarding without starting a
# server: bun is replaced by a recording stub at the process boundary, and the
# launcher is invoked through a published symlink exactly as the hook installs it.

LAUNCHER="${BATS_TEST_DIRNAME}/../toolu-claude"

setup() {
  TMP=$(mktemp -d)
  # Fake plugin layout: bin/toolu-claude (the real script) + dashboard/*.ts.
  mkdir -p "$TMP/plugin/bin" "$TMP/plugin/dashboard" "$TMP/path" "$TMP/binstub"
  cp "$LAUNCHER" "$TMP/plugin/bin/toolu-claude"
  chmod +x "$TMP/plugin/bin/toolu-claude"
  : > "$TMP/plugin/dashboard/index.ts"
  : > "$TMP/plugin/dashboard/config-cli.ts"
  # Publish the launcher as a symlink on a PATH dir, exactly like the hook does.
  ln -s "$TMP/plugin/bin/toolu-claude" "$TMP/path/toolu-claude"
  # Recording stub for bun: writes its argv (one per line) and exits 0.
  cat > "$TMP/binstub/bun" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$@" > "$TMP/bun-args"
EOF
  chmod +x "$TMP/binstub/bun"
  PATH="$TMP/path:$TMP/binstub:$PATH"
}

teardown() {
  [ -n "${TMP:-}" ] && [ -d "$TMP" ] && rm -rf "$TMP"
}

@test "dashboard --open forwards exactly to bun run <dash>/index.ts --open" {
  run toolu-claude dashboard --open
  [ "$status" -eq 0 ]
  printf '%s\n' "run" "$TMP/plugin/dashboard/index.ts" "--open" > "$TMP/expected"
  run diff "$TMP/expected" "$TMP/bun-args"
  [ "$status" -eq 0 ]
}

@test "dashboard config get forwards exactly to bun run <dash>/config-cli.ts get" {
  run toolu-claude dashboard config get
  [ "$status" -eq 0 ]
  printf '%s\n' "run" "$TMP/plugin/dashboard/config-cli.ts" "get" > "$TMP/expected"
  run diff "$TMP/expected" "$TMP/bun-args"
  [ "$status" -eq 0 ]
}

@test "no command prints usage and exits 1" {
  run toolu-claude
  [ "$status" -eq 1 ]
  echo "$output" | grep -qF "Usage:"
}

@test "unknown command prints actionable error and exits 1" {
  run toolu-claude frobnicate
  [ "$status" -eq 1 ]
  echo "$output" | grep -qF "unknown command 'frobnicate'"
}

@test "--help exits 0 and shows the dashboard usage" {
  run toolu-claude --help
  [ "$status" -eq 0 ]
  echo "$output" | grep -qF "toolu-claude dashboard"
}

@test "missing bun yields an actionable error and exit 127" {
  # coreutils present (dirname/readlink resolve) but bun absent.
  PATH="$TMP/path:/usr/bin:/bin"
  # Capture directly (not via `run`) so the expected 127 isn't flagged as a
  # suspicious "command not found" by bats, and so no run-flag version is needed.
  status=0
  output="$(toolu-claude dashboard 2>&1)" || status=$?
  [ "$status" -eq 127 ]
  echo "$output" | grep -qF "bun"
}

@test "a cyclic symlink chain aborts fast instead of hanging" {
  # Regression: the launcher caps symlink-resolution hops so a circular chain
  # (a -> b -> a) can never spin forever. Replace the published symlink with a
  # 2-node cycle and invoke through it.
  rm -f "$TMP/path/toolu-claude"
  ln -s "$TMP/path/cycle-b" "$TMP/path/toolu-claude"
  ln -s "$TMP/path/toolu-claude" "$TMP/path/cycle-b"

  # CRITICAL: wrap in `timeout` so a regression (infinite hop loop / kernel hang)
  # fails fast here instead of hanging the whole bats suite. `timeout` returns 124
  # only when it has to KILL the command — i.e. only when the launcher hung.
  # Invoke the cyclic link by ABSOLUTE path, not a bare `toolu-claude` PATH
  # lookup: a cyclic $0 can never exec the launcher (the kernel rejects it with
  # ELOOP), so a PATH lookup would fall through to any real toolu-claude the dev
  # has published on PATH (publish-cli does exactly that) and start a server —
  # making this test pass only on a pristine PATH. The absolute path is
  # hermetic: the loader rejects the cycle deterministically.
  status=0
  output="$(timeout 10 "$TMP/path/toolu-claude" dashboard 2>&1)" || status=$?

  # Must not hang: timeout never had to kill it.
  [ "$status" -ne 124 ]
  # Must abort non-zero rather than start a server.
  [ "$status" -ne 0 ]
  # The cyclic invocation never started a server: the bun stub was never reached.
  [ ! -f "$TMP/bun-args" ]
  # The failure is a symlink cycle: either the launcher's own guard fires
  # ("symlink resolution loop"), or the OS/loader rejects the cyclic invocation
  # first ("Too many levels of symbolic links", "No such file or directory",
  # or "command not found").
  echo "$output" | grep -qiE "symlink resolution loop|too many levels of symbolic links|no such file or directory|command not found"
}
