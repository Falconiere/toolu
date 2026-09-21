#!/usr/bin/env bash
# whole-session/run.sh — live headless A/B for the whole-session AGGREGATE. For
# each task, BASELINE runs `claude --bare` (toolu OFF, no plugins/hooks) and
# TREATMENT runs `claude` with all toolu plugins active (toolu ON). We compare
# BOTH total cost (from the `--output-format json` result's .total_cost_usd) AND
# total tokens (from stats_usage_rollup over the pinned-session transcript + its
# subagent transcripts). This is a whole-session AGGREGATE: it is NOT the sum of
# the per-mechanism deltas (see notes). A run whose `claude` invocation fails, or
# whose transcript is missing/empty, is DROPPED with a note (never a bogus
# sample); if no samples survive we abort nonzero.
set -u

_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$_dir/../../lib/common.sh"
# shellcheck source=/dev/null
source "$_dir/../../lib/result.sh"
# shellcheck source=/dev/null
source "$_dir/../../lib/tokens.sh"

# Resolve the transcript path Claude Code writes for a pinned (cwd, session-id):
#   <projects-root>/<slug>/<session>.jsonl  with subagents under
#   <projects-root>/<slug>/<session>/subagents/agent-*.jsonl
# slug = cwd with every non-alphanumeric char turned into '-' (mirrors stats).
whole_session_transcript_path() {  # $1=cwd $2=session-id -> transcript path
  local cwd="$1" sid="$2" root slug
  root="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/projects"
  slug="$(printf '%s' "$cwd" | sed 's/[^A-Za-z0-9]/-/g')"
  printf '%s/%s/%s.jsonl\n' "$root" "$slug" "$sid"
}

# Echo the transcript file followed by any subagent transcripts (one per line).
whole_session_files() {  # $1=transcript -> file list on stdout
  local t="$1" base f
  printf '%s\n' "$t"
  base="${t%.jsonl}"
  for f in "$base"/subagents/agent-*.jsonl; do
    [ -f "$f" ] && printf '%s\n' "$f"
  done
}

# Run one `claude` headless invocation. On success prints "<tokens>\t<cost_usd>"
# on stdout; non-zero on any failure (claude error, missing/empty transcript,
# missing cost field) so the caller can drop the run.
#   $1=cwd  $2=model  $3=task-text  $4=extra-flag ("" or "--bare")
whole_session_run_one() {
  local cwd="$1" model="$2" task="$3" extra="$4"
  local sid result cost transcript total roll
  sid="$(uuidgen | tr '[:upper:]' '[:lower:]')"

  # Run from $cwd so the transcript slug (derived from cwd) matches where Claude
  # Code actually writes — otherwise the transcript lands under a different slug
  # and every run is silently dropped.
  if [ -n "$extra" ]; then
    result="$( cd "$cwd" && claude -p "$extra" --output-format json --model "$model" --session-id "$sid" "$task" 2>/dev/null )" \
      || return 1
  else
    result="$( cd "$cwd" && claude -p --output-format json --model "$model" --session-id "$sid" "$task" 2>/dev/null )" \
      || return 1
  fi

  cost="$(printf '%s' "$result" | jq -r '.total_cost_usd // empty' 2>/dev/null)"
  [ -n "$cost" ] || return 1

  transcript="$(whole_session_transcript_path "$cwd" "$sid")"
  [ -s "$transcript" ] || return 1

  local files=()
  local f
  while IFS= read -r f; do
    [ -n "$f" ] && files+=("$f")
  done < <(whole_session_files "$transcript")
  roll="$(stats_usage_rollup "${files[@]}")" || return 1
  [ "$(printf '%s' "$roll" | jq -r '.messages')" -gt 0 ] 2>/dev/null || return 1
  total="$(printf '%s' "$roll" | jq -r '.totals.tokens')"
  [ -n "$total" ] && [ "$total" != "null" ] || return 1

  printf '%s\t%s\n' "$total" "$cost"
}

bench_whole_session_run() {
  # Parse args BEFORE the CLI gate so an unknown arg is a hard 2 regardless of
  # whether the claude CLI is installed (keeps the contract env-independent).
  local n=5 model="claude-sonnet-4-6" tasks="$_dir/tasks" cwd="$BENCH_ROOT"
  while [ $# -gt 0 ]; do
    case "$1" in
      --n)     n="${2:-}";     shift 2 ;;
      --model) model="${2:-}"; shift 2 ;;
      --tasks) tasks="${2:-}"; shift 2 ;;
      *) echo "whole-session: unknown arg: $1" >&2; return 2 ;;
    esac
  done

  command -v claude >/dev/null 2>&1 \
    || { echo "whole-session: live tier needs the claude CLI" >&2; return 1; }
  [ -d "$tasks" ] || { echo "whole-session: tasks dir not found: $tasks" >&2; return 1; }

  local cases='[]' tok_samples="" cost_samples="" notes=""
  local sum_base_tok=0 sum_treat_tok=0 n_pairs=0
  local base_cost_total=0 treat_cost_total=0
  local taskfile

  for taskfile in "$tasks"/*.txt; do
    [ -f "$taskfile" ] || continue
    local id task
    id="$(basename "$taskfile" .txt)"
    task="$(cat "$taskfile")"
    local b_tok_sum=0 t_tok_sum=0 b_n=0 t_n=0 run=1
    while [ "$run" -le "$n" ]; do
      local bout tout b_tok b_cost t_tok t_cost
      bout=""; tout=""
      if bout="$(whole_session_run_one "$cwd" "$model" "$task" "--bare")"; then
        b_tok="${bout%%$'\t'*}"; b_cost="${bout##*$'\t'}"
        b_tok_sum=$((b_tok_sum + b_tok)); b_n=$((b_n + 1))
      else
        notes="${notes}${id}#${run}:baseline-dropped; "
      fi
      if tout="$(whole_session_run_one "$cwd" "$model" "$task" "")"; then
        t_tok="${tout%%$'\t'*}"; t_cost="${tout##*$'\t'}"
        t_tok_sum=$((t_tok_sum + t_tok)); t_n=$((t_n + 1))
      else
        notes="${notes}${id}#${run}:treatment-dropped; "
      fi
      if [ -n "$bout" ] && [ -n "$tout" ]; then
        sum_base_tok=$((sum_base_tok + b_tok)); sum_treat_tok=$((sum_treat_tok + t_tok))
        tok_samples="${tok_samples}${t_tok}"$'\n'
        cost_samples="${cost_samples}${t_cost}"$'\n'
        base_cost_total="$(jq -n --argjson a "$base_cost_total"  --arg b "$b_cost" '$a + ($b|tonumber)')"
        treat_cost_total="$(jq -n --argjson a "$treat_cost_total" --arg b "$t_cost" '$a + ($b|tonumber)')"
        n_pairs=$((n_pairs + 1))
      fi
      run=$((run + 1))
    done
    cases="$(jq -c --arg id "$id" --argjson b "$b_tok_sum" --argjson t "$t_tok_sum" \
      --argjson bn "$b_n" --argjson tn "$t_n" \
      '. + [{id:$id, baseline_tokens:$b, treatment_tokens:$t, baseline_runs:$bn, treatment_runs:$tn}]' <<<"$cases")"
  done

  [ "$n_pairs" -gt 0 ] \
    || { echo "whole-session: no surviving paired samples (all runs dropped); refusing to write" >&2; return 1; }

  local tok_stats cost_stats tok_pct cost_pct abs_tok commit date
  tok_stats="$(printf '%s' "$tok_samples" | bench_stats)"
  cost_stats="$(printf '%s' "$cost_samples" | bench_stats)"
  tok_pct=0; [ "$sum_base_tok" -gt 0 ] && tok_pct=$(( (sum_base_tok - sum_treat_tok) * 100 / sum_base_tok ))
  abs_tok=$((sum_base_tok - sum_treat_tok))
  cost_pct="$(jq -n --argjson b "$base_cost_total" --argjson t "$treat_cost_total" \
    'if $b > 0 then (($b - $t) * 100 / $b) else null end')"
  commit="$(git -C "$BENCH_ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
  date="$(date +%Y-%m-%d)"

  jq -nc \
    --argjson cases "$cases" \
    --arg model "$model" --arg commit "$commit" --arg date "$date" --arg pid "$STATS_PRICING_ID" \
    --argjson nr "$n_pairs" \
    --argjson sb "$sum_base_tok" --argjson st "$sum_treat_tok" \
    --argjson bc "$base_cost_total" --argjson tc "$treat_cost_total" \
    --argjson tpct "$tok_pct" --argjson cpct "$cost_pct" --argjson abs "$abs_tok" \
    --argjson tstats "$tok_stats" --argjson cstats "$cost_stats" \
    --arg notes "$notes" \
    '{mechanism:"whole-session", tier:"live", method:"headless-ab",
      tokenizer:{mode:"usage", source:"usage"},
      provenance:{model:$model, date:$date, commit:$commit, n_runs:$nr, pricing_id:$pid},
      baseline: {label:"toolu-off", tokens:{input:null,output:null,cache_read:null,cache_write:null,total:$sb}, cost:$bc},
      treatment:{label:"toolu-on",  tokens:{input:null,output:null,cache_read:null,cache_write:null,total:$st}, cost:$tc},
      delta:{tokens_pct:$tpct, cost_pct:$cpct, abs_tokens:$abs, mean:$cstats.mean, stddev:$cstats.stddev},
      token_stats:$tstats, cost_stats:$cstats,
      cases:$cases,
      notes:($notes + "AGGREGATE whole-session delta (toolu-on vs --bare); NOT the sum of per-mechanism deltas."),
      _modes:["usage","usage"]}' \
    | bench_result_write
}

bench_whole_session_run "$@"
