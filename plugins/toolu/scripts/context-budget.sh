#!/usr/bin/env bash
# context-budget.sh — enforce context-window budgets on harness-injected text.
#
# Every session, Claude Code loads the Session Protocol + per-language docs and
# all skill `description` fields into the model's context. This guard caps that
# recurring footprint so it cannot silently regrow, and asserts that trimming
# never drops a skill's discriminating trigger phrases (which would stop it
# auto-firing).
#
# Fails CLOSED: a missing/renamed target file, or a description that can't be
# parsed, is RED — never a silent pass.
#
# Usage: context-budget.sh [docs|skills]   (no arg = all)
set -u

ROOT="${CONTEXT_BUDGET_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null)}"

fail=0
err() { printf 'RED  %s\n' "$1" >&2; fail=1; }
ok()  { printf 'ok   %s\n' "$1"; }

# count_words FILE -> prints word count; returns 1 if file missing.
count_words() {
  [ -f "$1" ] || return 1
  wc -w < "$1" | tr -d ' '
}

# extract_description FILE -> prints the YAML frontmatter `description` value.
# Handles single-line (`description: text`) and folded/block scalars
# (`description: >` followed by indented lines). Returns 1 if the file is absent.
extract_description() {
  [ -f "$1" ] || return 1
  awk '
    /^description:/ && !started {
      started=1
      v=$0; sub(/^description:[ \t]*/,"",v)
      if (v=="" || v==">" || v==">-" || v=="|" || v=="|-") { folded=1; next }
      print v; exit
    }
    folded {
      if ($0=="---") exit
      if ($0 ~ /^[A-Za-z0-9_-]+:/) exit
      sub(/^[ \t]+/,"")
      buf=(buf=="")?$0:buf" "$0
    }
    END { if (folded) print buf }
  ' "$1"
}

# check_doc NAME PATH BUDGET
check_doc() {
  local name="$1" path="$2" budget="$3" n
  n=$(count_words "$ROOT/$path") || { err "$name: MISSING $path"; return; }
  if [ "$n" -le "$budget" ]; then ok "$name ${n}w (<= ${budget})"
  else err "$name ${n}w EXCEEDS ${budget} ($path)"; fi
}

# check_skill NAME PATH BUDGET PHRASES(;-separated, may be empty)
check_skill() {
  local name="$1" path="$2" budget="$3" phrases="$4" desc n p
  desc=$(extract_description "$ROOT/$path") || { err "$name: MISSING $path"; return; }
  [ -n "$desc" ] || { err "$name: empty/unparseable description ($path)"; return; }
  n=$(printf '%s' "$desc" | wc -w | tr -d ' ')
  if [ "$n" -le "$budget" ]; then ok "$name desc ${n}w (<= ${budget})"
  else err "$name desc ${n}w EXCEEDS ${budget} ($path)"; fi
  local IFS=';'
  for p in $phrases; do
    [ -z "$p" ] && continue
    case "$desc" in
      *"$p"*) : ;;
      *) err "$name: missing trigger phrase \"$p\" ($path)" ;;
    esac
  done
}

run_docs() {
  check_doc session-start      plugins/toolu/hooks/docs/session-start.md      110
  check_doc model-routing      plugins/toolu/hooks/docs/model-routing.md       90
  check_doc post-compaction    plugins/toolu/hooks/docs/post-compaction.md     28
  check_doc session-start-ts   plugins/toolu/hooks/docs/session-start-ts.md    30
  check_doc session-start-rust plugins/toolu/hooks/docs/session-start-rust.md  36
}

run_skills() {
  # Trimmed skills: word ceiling + trigger phrases that MUST survive the trim.
  check_skill brainstorm       plugins/toolu/skills/brainstorm/SKILL.md       90 "where do I even start;help me scope it;think through the approach and tradeoffs"
  check_skill spec             plugins/toolu/skills/spec/SKILL.md             70 "write the spec;document the design"
  check_skill spec-review      plugins/toolu/skills/spec-review/SKILL.md      60 "review the spec;poke holes in this spec"
  check_skill deep-research    plugins/toolu/skills/deep-research/SKILL.md    90 "deep research;cited report"
  check_skill plan-review      plugins/toolu/skills/plan-review/SKILL.md      55 "review the plan;poke holes in the plan"
  check_skill execution-review plugins/toolu/skills/execution-review/SKILL.md 60 "review the execution;is this done"
  check_skill toolu-review      plugins/toolu-review/skills/review/SKILL.md         65 "review before push"
  # Already-lean skills: word ceiling only, lock against regrowth.
  check_skill plan             plugins/toolu/skills/plan/SKILL.md             60 ""
  check_skill execution        plugins/toolu/skills/execution/SKILL.md        60 ""
  check_skill test             plugins/toolu/skills/test/SKILL.md             60 ""
  check_skill agent-memory     plugins/comemory/skills/agent-memory/SKILL.md      30 ""
  check_skill project-skills   plugins/comemory/skills/project-skills/SKILL.md    60 "project skill;unused skill;.toolu/skills"
  check_skill ast-grep         plugins/ast-grep/skills/ast-grep/SKILL.md          40 ""
}

main() {
  case "${1:-all}" in
    docs)   run_docs ;;
    skills) run_skills ;;
    all)    run_docs; run_skills ;;
    *) echo "usage: context-budget.sh [docs|skills]" >&2; exit 2 ;;
  esac
  [ "$fail" -eq 0 ] || exit 1
}

if [ "${BASH_SOURCE[0]:-}" = "${0:-}" ]; then main "$@"; fi
