#!/usr/bin/env bash
# Live evidence for docs/toolu/plans/2026-10-27-cli-ux.md (macOS script(1) and
# expect(1)): the debug binary at a real terminal on this machine's real hosts.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
here="$root/docs/toolu/evidence"
toolu="$root/target/debug/toolu"
work=$(mktemp -d "${TMPDIR:-/tmp}/cli-ux-live.XXXXXX")
trap 'rm -rf "$work"' EXIT
cd "$root"
cargo build -q -p toolu-cli

fail() { echo "live-check: $*" >&2; exit 1; }
has() { LC_ALL=C grep -qF -- "$2" "$1" || fail "$1 lacks: $2"; }
clean() { LC_ALL=C sed 's/\x1b\[[0-9;]*[A-Za-z]//g' "$1" | tr '\r' '\n'; }

# 1. Rails, progress and its clear on a dry install, with no prompt to answer.
NO_COLOR=1 script -q "$work/install.raw" "$toolu" plugins install --host claude --dry-run --no-input </dev/null >/dev/null
clean "$work/install.raw" >"$work/install.txt"
has "$work/install.txt" "┌  toolu plugins install"
has "$work/install.txt" "◇  claude"
has "$work/install.txt" "└  "
has "$work/install.raw" "$(printf '\033[1A\r\033[2K')"

# 2. update targets exactly the @toolu plugins, once each; retired ones get a hint.
claude plugin list --json | jq -r '[.[].id | select(endswith("@toolu")) | sub("@toolu$"; "")] | unique | .[]' >"$work/toolu-ids"
"$toolu" plugins update --host claude --dry-run --json </dev/null >"$work/update.json"
jq -r '[.steps[].name] | sort | .[]' "$work/update.json" >"$work/steps"
diff "$work/toolu-ids" "$work/steps" || fail "update steps differ from the @toolu plugins"
for foreign in cloudflare comemory exa rust-analyzer-lsp ponytail caveman; do
  if grep -qx "$foreign" "$work/steps"; then fail "update touched $foreign"; fi
done
"$toolu" plugins list --host claude --json </dev/null | jq -r '.[].name' | sort >"$work/catalog"
comm -23 "$work/steps" "$work/catalog" >"$work/retired"
while read -r name; do
  jq -e --arg n "$name" '.steps[] | select(.name == $n) | select(.outcome == "skew" and (.detail | contains("--host claude --yes")))' "$work/update.json" >/dev/null || fail "$name is not reported retired"
done <"$work/retired"

# 3. One real update: the bar reaches 1/1 and jev is current or updated.
NO_COLOR=1 script -q "$work/jev.raw" "$toolu" plugins update jev --host claude </dev/null >/dev/null
clean "$work/jev.raw" >"$work/jev.txt"
has "$work/jev.txt" "1/1"
grep -Eq "jev +(current at|updated)" "$work/jev.txt" || { tail -5 "$work/jev.txt" >&2; fail "jev row missing"; }

# 4. The checkbox picker answers with arrows, space and Enter, then collapses.
expect "$here/2026-10-27-cli-ux-live.exp" "$toolu" pick >"$work/pick.raw" 2>&1
clean "$work/pick.raw" >"$work/pick.txt"
has "$work/pick.txt" "◆  Which hosts should receive the plugins?"
has "$work/pick.txt" "◼"
has "$work/pick.txt" "◻"
has "$work/pick.txt" "◇  Which hosts should receive the plugins?  "

# 5. q cancels with exit 1 and leaves the terminal as it was.
expect "$here/2026-10-27-cli-ux-live.exp" "$toolu" cancel >"$work/cancel.raw" 2>&1
clean "$work/cancel.raw" >"$work/cancel.txt"
has "$work/cancel.txt" "cancelled"
has "$work/cancel.txt" "rc=1"
has "$work/cancel.txt" "STTY_SAME"

# 6. status draws a table at a terminal, JSON in a pipe; color without NO_COLOR.
NO_COLOR=1 script -q "$work/status.raw" "$toolu" status </dev/null >/dev/null
clean "$work/status.raw" >"$work/status.txt"
has "$work/status.txt" "┌  toolu status"
has "$work/status.txt" "│  branch"
"$toolu" status </dev/null >"$work/status.json"
jq -e .branch "$work/status.json" >/dev/null || fail "piped status is not JSON"
if LC_ALL=C grep -q $'\x1b' "$work/status.json"; then fail "piped status has escapes"; fi
script -q "$work/color.raw" "$toolu" plugins list --host claude </dev/null >/dev/null
has "$work/color.raw" "$(printf '\033[32m')"

echo "live-check: ok"
