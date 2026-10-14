#!/usr/bin/env bash
# AC-8 evidence for #453: render the shipped dashboard in headless Chrome
# against two real `toolu epic serve` engines and check the fleet view.
#
# Engine B binds 0.0.0.0 with a status token. Engine A lists B twice (right
# and wrong token) and a closed port; engine C has no peers. Saves A's DOM
# beside this script.
# Manual check: needs Chrome, Bun and target/debug/toolu.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
toolu="$repo/target/debug/toolu"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
[ -x "$toolu" ] || { echo "missing $toolu: run cargo build -p toolu-cli" >&2; exit 2; }
[ -x "$chrome" ] || { echo "missing Chrome at $chrome (set CHROME)" >&2; exit 2; }

work="$(mktemp -d /tmp/fleet.XXXX)"
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  for pid in "${pids[@]}"; do wait "$pid" 2>/dev/null || true; done
  rm -rf "$work"
}
trap cleanup EXIT INT TERM HUP

free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'
}

# seed <dir> <epic> <issue>: a registered epic whose graph holds one issue.
seed() {
  mkdir -p "$1/home" "$1/res" "$1/cfg" "$1/$2/status"
  printf '{"epics":[{"key":"%s","state_dir":"%s"}]}\n' "$2" "$1/$2" > "$1/res/registry.json"
  printf '{"issues":[{"key":"%s","title":"Fleet evidence","wave":1,"open_blockers":[]}]}\n' \
    "$3" > "$1/$2/graph.json"
}

# engine <dir> <bind> [env...]: start `toolu epic serve` and wait for HTTP.
engine() {
  local dir="$1" bind="$2"
  shift 2
  env HOME="$dir/home" TOOLU_RESOURCE_HOME="$dir/res" TOOLU_CONFIG_DIR="$dir/cfg" "$@" \
    "$toolu" epic serve --http "$bind" > "$dir/engine.log" 2>&1 &
  pids+=("$!")
  local port="${bind##*:}"
  for _ in $(seq 1 200); do
    if curl -s -o /dev/null "http://127.0.0.1:$port/healthz" -H "Host: 127.0.0.1:$port"; then return 0; fi
    sleep 0.05
  done
  echo "engine on $bind did not start" >&2
  cat "$dir/engine.log" >&2
  exit 1
}

# report <dir> <epic> <issue>: record execution so the issue runs.
report() {
  env HOME="$1/home" TOOLU_RESOURCE_HOME="$1/res" TOOLU_CONFIG_DIR="$1/cfg" \
    "$toolu" epic report --status-file "$1/$2/status/$3.json" execution --note evidence > /dev/null
}

a="$work/a"; b="$work/b"; c="$work/c"
pa="$(free_port)"; pb="$(free_port)"; pc="$(free_port)"; closed="$(free_port)"
seed "$b" repo-500 repo-501
engine "$b" "0.0.0.0:$pb" TOOLU_EPIC_STATUS_TOKEN=evidence-right
report "$b" repo-500 repo-501

seed "$a" repo-402 repo-449
cat > "$a/cfg/toolu.config.json" <<JSON
{"version":1,"epic":{"peers":[
  {"name":"server","url":"http://127.0.0.1:$pb"},
  {"name":"vault","url":"http://127.0.0.1:$pb"},
  {"name":"nas","url":"http://127.0.0.1:$closed"}]}}
JSON
engine "$a" "127.0.0.1:$pa" \
  TOOLU_EPIC_PEER_TOKENS='{"server":"evidence-right","vault":"evidence-wrong"}'
report "$a" repo-402 repo-449
seed "$c" repo-600 repo-601
engine "$c" "127.0.0.1:$pc"

bun_bin="${TOOLU_BUN:-$(command -v bun || echo "$HOME/.bun/bin/bun")}"
# dump <port> [tab]: the rendered page shell (no script text) after 3 s,
# after clicking machine tab <tab> when given.
dump() {
  "$bun_bin" "$here/epic-fleet-dashboard-dom.ts" "$chrome" "http://127.0.0.1:$1/" 3000 "${2:-}"
}
dump "$pa" > "$here/epic-fleet-dashboard.html"
peer_page="$(curl -s "http://127.0.0.1:$pa/api/v1/fleet/peers/server/issues/repo-501")"
dump "$pc" > "$work/lone.html"
dump "$pa" server > "$work/server.html"
dump "$pa" vault > "$work/vault.html"

fail=0
check() {
  if grep -qF -- "$2" "$1"; then echo "ok: $3"; else echo "FAIL: $3" >&2; fail=1; fi
}
page="$here/epic-fleet-dashboard.html"
check "$page" '<dl id="fleet-summary" class="summary">' "fleet summary shown"
check "$page" '<dd id="fleet-machines">2 / 4</dd>' "2 of 4 machines reachable"
check "$page" 'This machine' "local tab"
check "$page" '>server<small>Live</small>' "server tab live"
check "$page" 'Unauthorized: token rejected' "wrong-token peer"
check "$page" 'Unreachable · never seen' "closed peer"
check "$page" 'repo-449' "local issue on the board"
printf '%s' "$peer_page" > "$work/peer.json"
check "$work/peer.json" '"key":"repo-501"' "peer issue through the proxy"
check "$work/server.html" '<h1 id="title">Epic status · server</h1>' "server view selected"
check "$work/server.html" 'repo-501' "server board through the proxy"
check "$work/vault.html" 'vault: Unauthorized: token rejected' "vault banner instead of panels"
check "$work/vault.html" '<div id="panels" hidden="">' "vault panels hidden"
check "$work/lone.html" '<div id="machines" class="tabs machines" role="tablist" aria-label="Machines" hidden="">' "no peers: switcher hidden"
check "$work/lone.html" '<dl id="fleet-summary" class="summary" hidden="">' "no peers: fleet summary hidden"
exit "$fail"
