#!/usr/bin/env bash
# render_html.sh — render the aggregate object as a self-contained HTML report by
# filling templates/report.html. Values are substituted with bash literal
# replacement (${tmpl//"{{K}}"/$v}) so paths/specials in the data never get
# reinterpreted (unlike sed); data is HTML-escaped via jq @html. Requires the
# token/cost humanizers from render.sh (stats_format_tokens, stats_money) to be
# sourced — stats.sh sources both. The browser-open side effect is guarded by
# STATS_NO_OPEN=1 (set in tests).
set -u

# Disable bash 5.2+ patsub_replacement so `&` (and `\`) in ${var//pat/repl} values
# are literal, not the matched text. No-op (option unknown) on older bash —
# including bash 5.0/5.1 and macOS /bin/bash 3.2 — where they are already literal.
shopt -u patsub_replacement 2>/dev/null || true

# Absolute path of the report file (under the stats config dir).
stats_html_path() { printf '%s/stats/report.html' "${CLAUDE_CONFIG_DIR:-$HOME/.claude}"; }

# Substitute {{KEY}} with VALUE in the caller's $tmpl (dynamic scope). With
# patsub_replacement disabled (top of file), & and \ are literal in the
# replacement on every bash version, so arbitrary HTML/data substitutes cleanly.
_stats_apply() {  # $1=KEY $2=VALUE
  tmpl="${tmpl//"{{$1}}"/$2}"
}

# Build one HTML block of <div class="row"> from a jq expression that emits
# "tokens\tcost\tname" lines (name already @html-escaped, rows sorted desc). The
# bar width % is scaled to the section's largest row, computed in bash.
_stats_html_rows() {  # $1=agg $2=jq-rows
  local agg="$1" expr="$2" tk co nm out="" pct
  local -a rows=(); local line max=0
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    rows+=("$line")
    tk="${line%%$'\t'*}"
    [[ "$tk" =~ ^[0-9]+$ ]] && [ "$tk" -gt "$max" ] && max="$tk"
  done < <(printf '%s' "$agg" | jq -r "$expr")
  [ "${#rows[@]}" -eq 0 ] && return 0
  for line in "${rows[@]}"; do
    IFS=$'\t' read -r tk co nm <<<"$line"
    if [ "$max" -gt 0 ]; then pct=$(( tk * 100 / max )); else pct=0; fi
    out+="    <div class=\"row\"><span class=\"name\">${nm}</span>"
    out+="<span class=\"track\"><span class=\"fill\" style=\"width:${pct}%\"></span></span>"
    out+="<span class=\"tok\">$(stats_format_tokens "$tk")</span>"
    out+="<span class=\"cost\">\$$(stats_money "$co")</span></div>"$'\n'
  done
  printf '%s' "$out"
}

# Build an inline SVG area+line chart from the daily series (date,tokens): a
# gradient-filled area under a 2.5px line, faint gridlines, the peak day marked
# with a value bubble, and first/last date ticks. The default markup is the full
# chart; the template's CSS only animates it (line draw) when JS opts in, so a
# no-JS / headless render still shows everything. A muted note when daily is
# null (i.e. under a --model filter).
_stats_html_spark() {  # $1=agg
  local agg="$1"
  if [ "$(printf '%s' "$agg" | jq -r '.daily == null')" = "true" ]; then
    printf '<p class="tmuted">Per-day trend is unavailable under a model filter.</p>'
    return 0
  fi
  local -a dts=() vals=() costs=(); local d v c max=0 i n
  while IFS=$'\t' read -r d v c; do
    dts+=("$d"); vals+=("$v"); costs+=("$c")
    [[ "$v" =~ ^[0-9]+$ ]] && [ "$v" -gt "$max" ] && max="$v"
  done < <(printf '%s' "$agg" | jq -r '.daily[] | "\(.date)\t\(.tokens)\t\(.cost)"')
  n="${#vals[@]}"
  [ "$n" -ge 2 ] || { printf '<p class="tmuted">Not enough days to chart yet.</p>'; return 0; }

  local W=1000 padL=8 padR=8 padT=18 plotH=152 baseY=170
  local plotW=$(( W - padL - padR ))
  local -a xs=() ys=(); local x y peakx=0 peaky=0 seen=0
  for i in "${!vals[@]}"; do
    v="${vals[$i]}"; [[ "$v" =~ ^[0-9]+$ ]] || v=0
    x=$(( padL + i * plotW / (n - 1) ))
    if [ "$max" -gt 0 ]; then y=$(( padT + plotH - v * plotH / max )); else y=$baseY; fi
    xs+=("$x"); ys+=("$y")
    if [ "$max" -gt 0 ] && [ "$v" -eq "$max" ] && [ "$seen" -eq 0 ]; then seen=1; peakx=$x; peaky=$y; fi
  done

  local line="M ${xs[0]} ${ys[0]}" area="M ${xs[0]} $baseY L ${xs[0]} ${ys[0]}"
  for ((i = 1; i < n; i++)); do line+=" L ${xs[$i]} ${ys[$i]}"; area+=" L ${xs[$i]} ${ys[$i]}"; done
  area+=" L ${xs[$((n - 1))]} $baseY Z"

  local grid="" gy
  for gy in $(( padT + plotH / 4 )) $(( padT + plotH / 2 )) $(( padT + plotH * 3 / 4 )) "$baseY"; do
    grid+="<line class=\"grid\" x1=\"$padL\" y1=\"$gy\" x2=\"$(( W - padR ))\" y2=\"$gy\"></line>"
  done

  # Per-day hover targets: a transparent full-height band + a (hidden) marker dot
  # per day, each carrying that day's figures for the JS tooltip.
  local dots="" hits="" bl br
  for i in "${!xs[@]}"; do
    if [ "$i" -eq 0 ]; then bl=0; else bl=$(( (xs[i-1] + xs[i]) / 2 )); fi
    if [ "$i" -eq $((n - 1)) ]; then br=$W; else br=$(( (xs[i] + xs[i+1]) / 2 )); fi
    dots+="<circle class=\"dot\" data-i=\"$i\" cx=\"${xs[i]}\" cy=\"${ys[i]}\" r=\"3.5\"></circle>"
    hits+="<rect class=\"hit\" data-i=\"$i\" data-d=\"${dts[i]:5}\" data-t=\"$(stats_format_tokens "${vals[i]}")\" data-c=\"\$$(stats_money "${costs[i]}")\" x=\"$bl\" y=\"0\" width=\"$(( br - bl ))\" height=\"200\"></rect>"
  done

  local plab="" lx
  if [ "$seen" -eq 1 ]; then
    lx=$peakx; [ "$lx" -lt 60 ] && lx=60; [ "$lx" -gt $(( W - 60 )) ] && lx=$(( W - 60 ))
    plab="<text class=\"plab\" x=\"$lx\" y=\"$(( peaky - 9 ))\" text-anchor=\"middle\">$(stats_format_tokens "$max")</text>"
    plab+="<circle class=\"peak\" cx=\"$peakx\" cy=\"$peaky\" r=\"4\"></circle>"
  fi
  local xlabs="<text class=\"xlab\" x=\"$padL\" y=\"190\" text-anchor=\"start\">${dts[0]:5}</text>"
  xlabs+="<text class=\"xlab\" x=\"$(( W - padR ))\" y=\"190\" text-anchor=\"end\">${dts[$((n - 1))]:5}</text>"

  printf '<svg class="spark" viewBox="0 0 %s 200" role="img" aria-label="token usage, last %s days"><defs><linearGradient id="sparkfill" x1="0" y1="0" x2="0" y2="1"><stop class="g0" offset="0"></stop><stop class="g1" offset="1"></stop></linearGradient></defs>%s<path class="area" d="%s"></path><path class="line" pathLength="1" d="%s"></path>%s%s%s%s</svg>' \
    "$W" "$n" "$grid" "$area" "$line" "$plab" "$xlabs" "$dots" "$hits"
}

# Open the report in the default browser unless suppressed. Never fatal.
_stats_open_html() {  # $1=path
  [ "${STATS_NO_OPEN:-0}" = "1" ] && return 0
  if   command -v open     >/dev/null 2>&1; then open "$1"     >/dev/null 2>&1 || true
  elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$1" >/dev/null 2>&1 || true
  fi
}

# stats_render_html < aggregate.json -> writes the report, prints its path.
stats_render_html() {
  local agg; agg="$(cat)"
  local sess; sess="$(printf '%s' "$agg" | jq -r '.totals.sessions')"
  if [ "${sess:-0}" -eq 0 ] 2>/dev/null; then
    echo "stats: no usage recorded yet."
    return 0
  fi

  local tmpl_path; tmpl_path="${STATS_TEMPLATE:-$(dirname "${BASH_SOURCE[0]}")/../../templates/report.html}"
  if [ ! -r "$tmpl_path" ]; then
    echo "stats: HTML template not found at $tmpl_path" >&2
    return 1
  fi
  local tmpl; tmpl="$(cat "$tmpl_path")"

  local gen tok cost hit today week all
  IFS=$'\t' read -r gen tok cost hit today week all < <(printf '%s' "$agg" | jq -r \
    '[ .generated_at, (.totals.tokens|tostring), (.totals.cost|tostring), (.totals.cache_hit_pct|tostring),
       ((.windows.today.tokens // 0)|tostring), ((.windows.week.tokens // 0)|tostring), ((.windows.all.tokens // 0)|tostring) ] | @tsv')

  # Tools/phases render as pills (top 16 by count); key/value are @html-escaped
  # but the <span> wrappers are intentional markup substituted verbatim.
  local tools phases gate comem
  local chip='to_entries | sort_by(-.value) | .[0:16]
    | map("<span class=\"chip\"><b>"+(.key|@html)+"</b><i>"+(.value|tostring)+"</i></span>")
    | join("") | if .=="" then "<span class=\"tmuted\">—</span>" else . end'
  tools="$(printf '%s' "$agg"  | jq -r ".activity.tools  | $chip")"
  phases="$(printf '%s' "$agg" | jq -r ".activity.phases | $chip")"
  gate="$(printf '%s' "$agg"   | jq -r '.activity.gate | @html')"
  comem="$(printf '%s' "$agg"  | jq -r '.activity.comemory | tostring')"

  # Projects are capped to the top 8 by tokens; the long tail collapses into one
  # "+N more" row so the dashboard stays a dashboard, not a 75-row dump.
  local prows mrows srows spark
  prows="$(_stats_html_rows "$agg" '([.by_project[]]|sort_by(-.tokens)) as $p
    | ($p[0:8][] | "\(.tokens)\t\(.cost)\t\(.project|@html)")
    , (($p[8:]) as $r | if ($r|length) > 0
        then "\([$r[].tokens]|add)\t\([$r[].cost]|add)\t\(("+"+($r|length|tostring)+" more projects")|@html)"
        else empty end)')"
  mrows="$(_stats_html_rows "$agg" '.by_model[]     | "\(.tokens)\t\(.cost)\t\((.model|sub("^claude-";""))|@html)"')"
  srows="$(_stats_html_rows "$agg" '.top_sessions[] | "\(.tokens)\t\(.cost)\t\((.project + " " + .session_id[0:8])|@html)"')"
  spark="$(_stats_html_spark "$agg")"

  # Section subtotals shown to the right of each header (informative, in place
  # of a decorative label): project spend + count, model count, sessions shown.
  local pcount pcost mcount scount pmeta mmeta smeta
  IFS=$'\t' read -r pcount pcost mcount scount < <(printf '%s' "$agg" | jq -r \
    '[ (.by_project|length|tostring), ([.by_project[].cost]|add // 0|tostring),
       (.by_model|length|tostring), (.top_sessions|length|tostring) ] | @tsv')
  pmeta="\$$(stats_money "$pcost") · $pcount project$([ "$pcount" = "1" ] || echo s)"
  mmeta="$mcount model$([ "$mcount" = "1" ] || echo s)"
  smeta="top $scount of $sess"

  _stats_apply GENERATED_AT "$gen"
  _stats_apply TOTAL_TOKENS "$(stats_format_tokens "$tok")"
  _stats_apply TOTAL_TOKENS_RAW "$tok"
  _stats_apply TOTAL_COST   "$(stats_money "$cost")"
  _stats_apply TOTAL_COST_RAW "$cost"
  _stats_apply CACHE_PCT    "$hit"
  _stats_apply SESSIONS     "$sess"
  _stats_apply TODAY_TOKENS "$(stats_format_tokens "$today")"
  _stats_apply WEEK_TOKENS  "$(stats_format_tokens "$week")"
  _stats_apply ALL_TOKENS   "$(stats_format_tokens "$all")"
  _stats_apply SPARKLINE_SVG "$spark"
  _stats_apply PROJECT_ROWS "$prows"
  _stats_apply MODEL_ROWS   "$mrows"
  _stats_apply SESSION_ROWS "$srows"
  _stats_apply PROJECTS_META "$pmeta"
  _stats_apply MODELS_META   "$mmeta"
  _stats_apply SESSIONS_META "$smeta"
  _stats_apply TOOLS    " $tools"
  _stats_apply PHASES   " $phases"
  _stats_apply GATE     " $gate"
  _stats_apply COMEMORY " $comem"

  local out; out="$(stats_html_path)"
  mkdir -p "$(dirname "$out")" || { echo "stats: cannot create $(dirname "$out")" >&2; return 1; }
  printf '%s\n' "$tmpl" > "$out" || { echo "stats: cannot write $out" >&2; return 1; }
  printf 'Wrote HTML report: %s\n' "$out"
  _stats_open_html "$out"
}
