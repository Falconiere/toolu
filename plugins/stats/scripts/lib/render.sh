#!/usr/bin/env bash
# render.sh — present the aggregate object as a glyph dashboard (default),
# raw JSON (STATS_OUTPUT=json), or an HTML report (STATS_OUTPUT=html, via
# stats_render_html which stats.sh sources). The dashboard is box-drawing +
# bars + a sparkline, no ANSI, so it renders identically wherever it is shown.
# Token counts are humanized (13.7M / 45k); costs are 2-decimal estimates.
set -u

# shellcheck source=widgets.sh
source "$(dirname "${BASH_SOURCE[0]}")/widgets.sh"

# 13779513 -> 13.7M, 45000 -> 45k, else the raw integer. Integer-only math so it
# is safe on any input (a non-numeric value renders as 0).
stats_format_tokens() {
  local n="$1"
  [[ "$n" =~ ^[0-9]+$ ]] || { printf '0'; return; }
  if   [ "$n" -ge 1000000 ]; then printf '%d.%dM' "$(( n / 1000000 ))" "$(( (n % 1000000) / 100000 ))"
  elif [ "$n" -ge 1000 ];    then printf '%dk' "$(( n / 1000 ))"
  else printf '%d' "$n"; fi
}

# Format a dot-decimal cost as 2dp regardless of locale. printf '%.2f' rejects a
# dot-input under a comma-decimal locale, so pin LC_ALL=C for this call only —
# the rest of the renderer stays in the user's (UTF-8) locale so character-width
# math for the box/bars is correct.
stats_money() { local out; LC_ALL=C printf -v out '%.2f' "$1"; printf '%s' "$out"; }

# Read the aggregate JSON (stdin) and print it.
stats_render() {
  local agg; agg="$(cat)"
  if [ "${STATS_OUTPUT:-text}" = "json" ]; then printf '%s\n' "$agg" | jq .; return 0; fi
  if [ "${STATS_OUTPUT:-text}" = "html" ]; then printf '%s' "$agg" | stats_render_html; return 0; fi

  local tok cost hit sess
  IFS=' ' read -r tok cost hit sess < <(printf '%s' "$agg" | jq -r '.totals | "\(.tokens) \(.cost) \(.cache_hit_pct) \(.sessions)"')
  if [ "${sess:-0}" -eq 0 ] 2>/dev/null; then
    echo "stats: no usage recorded yet."
    return 0
  fi

  # Boxed header: economics + a cache-hit gauge.
  local inner=50
  stats_box_top "$inner" "Claude Code Usage"; printf '\n'
  stats_box_line "$inner" "$(printf '%s tokens   $%s   %s sessions' "$(stats_format_tokens "$tok")" "$(stats_money "$cost")" "$sess")"; printf '\n'
  stats_box_line "$inner" "$(printf 'cache hit %s%%  %s' "$hit" "$(stats_gauge "$hit" 20)")"; printf '\n'
  stats_box_bottom "$inner"; printf '\n'

  # 14-day sparkline + today/week headline (absent under a --model filter, where
  # windows and daily are null because days can't be sliced by model).
  if [ "$(printf '%s' "$agg" | jq -r '.windows == null')" = "true" ]; then
    printf '\n  Trend & windows: n/a under --model filter\n'
  else
    local -a dvals=(); local d
    while IFS= read -r d; do dvals+=("$d"); done < <(printf '%s' "$agg" | jq -r '.daily[].tokens')
    local tt wt
    IFS=' ' read -r tt wt < <(printf '%s' "$agg" | jq -r '.windows | "\(.today.tokens) \(.week.tokens)"')
    printf '\n  Trend 14d  %s    today %s · wk %s\n' \
      "$(stats_sparkline "${dvals[@]}")" "$(stats_format_tokens "$tt")" "$(stats_format_tokens "$wt")"
  fi

  _stats_render_bars "$agg" 'Projects'     '.by_project[]   | "\(.tokens)\t\(.cost)\t\(.sessions)\t\(.project)"'
  _stats_render_bars "$agg" 'Models'       '.by_model[]     | "\(.tokens)\t\(.cost)\t-\t\(.model)"'
  _stats_render_bars "$agg" 'Top sessions' '.top_sessions[] | "\(.tokens)\t\(.cost)\t-\t\(.project) \(.session_id[0:8])"'

  local tools phases gate comem
  tools="$(printf '%s' "$agg"  | jq -r '.activity.tools  | to_entries | map("\(.key):\(.value)") | join(" ") | if .=="" then "-" else . end')"
  phases="$(printf '%s' "$agg" | jq -r '.activity.phases | to_entries | map("\(.key):\(.value)") | join(" ") | if .=="" then "-" else . end')"
  gate="$(printf '%s' "$agg"   | jq -r '.activity.gate')"
  comem="$(printf '%s' "$agg"  | jq -r '.activity.comemory')"
  printf '\n  Activity  tools: %s · phases: %s · gate: %s · comemory: %s\n' \
    "$tools" "$phases" "$gate" "$comem"

  # Surface pricing warnings (e.g. an unknown model priced at the Sonnet default)
  # so a silent mispricing becomes visible rather than buried in the estimate.
  if [ "$(printf '%s' "$agg" | jq -r '(.warnings // []) | length')" -gt 0 ] 2>/dev/null; then
    printf '\n  ⚠ pricing warnings:\n'
    printf '%s' "$agg" | jq -r '(.warnings // [])[] | "    - " + .'
  fi
}

# Render one labelled section as bars, scaled to the section's largest row.
# Rows arrive as "tokens\tcost\tsess\tname" (sess "-" when not applicable),
# already sorted descending by tokens.
_stats_render_bars() {  # $1=agg $2=title $3=jq-rows
  local agg="$1" title="$2" expr="$3"
  local -a rows=(); local line max=0 tk
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    rows+=("$line")
    tk="${line%%$'\t'*}"
    [[ "$tk" =~ ^[0-9]+$ ]] && [ "$tk" -gt "$max" ] && max="$tk"
  done < <(printf '%s' "$agg" | jq -r "$expr")
  [ "${#rows[@]}" -eq 0 ] && return 0

  printf '\n  %s\n' "$title"
  local tk2 co cn nm
  for line in "${rows[@]}"; do
    IFS=$'\t' read -r tk2 co cn nm <<<"$line"
    [ "$(_stats_dwidth "$nm")" -gt 22 ] && nm="${nm:0:21}…"
    if [ "$cn" = "-" ]; then
      printf '  %s %s  %7s  %8s\n' "$(_stats_pad "$nm" 22)" "$(stats_bar "$tk2" "$max" 14)" "$(stats_format_tokens "$tk2")" "\$$(stats_money "$co")"
    else
      printf '  %s %s  %7s  %8s  %3s sess\n' "$(_stats_pad "$nm" 22)" "$(stats_bar "$tk2" "$max" 14)" "$(stats_format_tokens "$tk2")" "\$$(stats_money "$co")" "$cn"
    fi
  done
}
