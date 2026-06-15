#!/usr/bin/env bats
# render_html.sh — fills templates/report.html from a crafted aggregate (reusing
# aggregate.sh + render.sh humanizers). Browser-open suppressed via STATS_NO_OPEN.

setup() {
  source "${BATS_TEST_DIRNAME}/../scripts/lib/aggregate.sh"
  source "${BATS_TEST_DIRNAME}/../scripts/lib/render.sh"
  source "${BATS_TEST_DIRNAME}/../scripts/lib/render_html.sh"
  export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/cfg"
  export STATS_NO_OPEN=1
  TODAY="$(date +%Y-%m-%d)"
  full() { echo "{\"tokens\":$1,\"input\":$1,\"output\":0,\"cache_read\":0,\"cache_write\":0,\"cost\":$2}"; }
  # Project name carries HTML-special chars to exercise escaping.
  ROLLUPS=$(cat <<EOF
[
 {"session_id":"sess-aaaa1111","project":"a & b <x>","project_path":"/p/a",
  "totals":$(full 1500000 4.20),
  "by_day":{"$TODAY":$(full 1500000 4.20)},
  "by_model":{"claude-opus-4-8":$(full 1500000 4.20)},
  "tools":{"Read":5},"phases":{"execution":3}}
]
EOF
)
  AGG="$(echo "$ROLLUPS" | stats_aggregate)"
  REPORT="$CLAUDE_CONFIG_DIR/stats/report.html"
}

html() { echo "$AGG" | stats_render_html; }

@test "writes a self-contained report and prints its path" {
  run html
  [ "$status" -eq 0 ]
  [ -f "$REPORT" ]
  echo "$output" | grep -qF "$REPORT"
  grep -q "<!DOCTYPE html>" "$REPORT"
  grep -q "Claude Code Usage" "$REPORT"
  grep -q "<html" "$REPORT"
}

@test "leaves no unsubstituted placeholder" {
  html
  run grep -qF "{{" "$REPORT"; [ "$status" -ne 0 ]
}

@test "HTML-escapes special characters in names" {
  html
  grep -qF "a &amp; b &lt;x&gt;" "$REPORT"
  run grep -qF "<x>" "$REPORT"; [ "$status" -ne 0 ]
}

@test "entities survive substitution with patsub_replacement off (bash <5.2)" {
  shopt -u patsub_replacement 2>/dev/null || true   # simulate bash 5.0/5.1/3.2
  html
  grep -qF "a &amp; b &lt;x&gt;" "$REPORT"
  run grep -qF '\&amp;' "$REPORT"; [ "$status" -ne 0 ]   # not corrupted to literal backslash-entity
}

@test "draws CSS bar fills and an inline SVG area+line chart" {
  html
  grep -qF 'class="fill" style="width:' "$REPORT"
  grep -qF '<svg class="spark"' "$REPORT"
  grep -qF '<path class="area"' "$REPORT"
  grep -qF '<path class="line"' "$REPORT"
}

@test "humanizes totals and shows the cache gauge width" {
  html
  grep -qF "1.5M" "$REPORT"
  grep -qF "width:0%" "$REPORT"   # cache hit 0% for this rollup
}

@test "under --model the sparkline degrades to a note" {
  export STATS_MODEL=opus; AGG="$(echo "$ROLLUPS" | stats_aggregate)"; unset STATS_MODEL
  html
  grep -qF "unavailable under a model filter" "$REPORT"
}

@test "errors non-zero when the template is missing" {
  STATS_TEMPLATE="$BATS_TEST_TMPDIR/nope.html" run html
  [ "$status" -ne 0 ]
  echo "$output" | grep -qF "template not found"
}

@test "empty usage prints a notice and writes nothing" {
  AGG="$(echo '[]' | stats_aggregate)"
  run html
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "no usage recorded"
  [ ! -f "$REPORT" ]
}
