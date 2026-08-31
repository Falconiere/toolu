#!/usr/bin/env bash
# conventions-detect — build a repo convention profile (JSON) from declared
# files + inferred git history + (best-effort) gh. Pure bash; zero model tokens.
# Usage: conventions-detect.sh [repo_root]   → profile JSON on stdout.
# NOTE: no `pipefail` — many pipelines end in `head`/optional `grep` whose
# upstream SIGPIPE or no-match would otherwise trip `set -e` mid-assignment.
set -eu

srchash_only=0
if [ "${1:-}" = "--source-hash" ]; then srchash_only=1; shift; fi
root="${1:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
command -v jq >/dev/null 2>&1 || { echo '{"error":"jq missing"}'; exit 0; }

_sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi; }

# ── declared convention files (existing only) ───────────────────────────────
declared_files() {
  local p g
  for p in .github/PULL_REQUEST_TEMPLATE.md .github/pull_request_template.md \
           CONTRIBUTING.md .czrc cz.json .releaserc release-please-config.json \
           .github/release.yml .gitmessage CODEOWNERS .github/CODEOWNERS; do
    [ -f "$root/$p" ] && printf '%s\n' "$root/$p"
  done
  for g in "$root"/.github/PULL_REQUEST_TEMPLATE/*.md "$root"/.github/ISSUE_TEMPLATE/* \
           "$root"/commitlint* "$root"/.releaserc.*; do
    [ -f "$g" ] && printf '%s\n' "$g"
  done
  return 0   # last `[ -f ]` may be false; don't let that fail the function
}
src_hash=$(declared_files | sort | xargs cat 2>/dev/null | _sha256)
[ "$srchash_only" = 1 ] && { printf '%s\n' "$src_hash"; exit 0; }

# ── commit format (inferred from last 50 subjects) ──────────────────────────
# Window = 50 (not 100): the convention is decided by majority over recent
# commits, so a smaller recent window tracks the repo's CURRENT practice. A
# repo that recently adopted conventional commits is detected at 50; a larger
# window would dilute it with older pre-convention history and mis-report.
subjects=$(git -C "$root" log -n 50 --format=%s 2>/dev/null || true)
total=$(printf '%s' "$subjects" | grep -c . || true)
conv_re='^(feat|fix|chore|docs|test|refactor|perf|build|ci|style|revert)(\([^)]+\))?!?: '
conv_count=$(printf '%s\n' "$subjects" | grep -cE "$conv_re" || true)
if [ "$total" -gt 0 ] && [ "$((conv_count * 2))" -ge "$total" ]; then
  convention="conventional-commits"
else
  convention="unknown"
fi
types_json=$(printf '%s\n' "$subjects" | sed -nE 's/^([a-z]+)(\([^)]+\))?!?: .*/\1/p' | sort -u | jq -R . | jq -sc .)
scope="none"; printf '%s\n' "$subjects" | grep -qE '^[a-z]+\([^)]+\)!?: ' && scope="used"
pr_suffix=null; printf '%s\n' "$subjects" | grep -qE ' \(#[0-9]+\)$' && pr_suffix='"(#N)"'
samples_json=$(printf '%s\n' "$subjects" | grep . | head -3 | jq -R . | jq -sc .)

# ── branch naming (inferred) ────────────────────────────────────────────────
branches=$(git -C "$root" branch -a --format='%(refname:short)' 2>/dev/null | sed 's#^origin/##' | grep -vE '^(HEAD|$)' | sort -u || true)
btotal=$(printf '%s' "$branches" | grep -c . || true)
bslash=$(printf '%s\n' "$branches" | grep -cE '^[a-z]+/' || true)
if [ "$btotal" -gt 0 ] && [ "$((bslash * 2))" -ge "$btotal" ]; then
  bpattern="type/kebab"
else
  bpattern="unknown"
fi
bprefixes_json=$(printf '%s\n' "$branches" | sed -nE 's#^([a-z]+)/.*#\1#p' | sort -u | jq -R . | jq -sc .)
bexamples_json=$(printf '%s\n' "$branches" | grep -E '^[a-z]+/' | head -2 | jq -R . | jq -sc .)

# ── PR ──────────────────────────────────────────────────────────────────────
pr_template=null; pr_sections_json='[]'
for t in .github/PULL_REQUEST_TEMPLATE.md .github/pull_request_template.md; do
  if [ -f "$root/$t" ]; then
    pr_template="\"$t\""
    pr_sections_json=$(grep -E '^#{1,3} +' "$root/$t" | sed -E 's/^#+ +//' | jq -R . | jq -sc .)
    break
  fi
done

# ── gh recent PR titles (best-effort, bounded) ──────────────────────────────
gh_available=false; recent_titles_json='[]'
if command -v gh >/dev/null 2>&1; then
  gh_available=true
  TO=""
  command -v timeout >/dev/null 2>&1 && TO="timeout 5"
  command -v gtimeout >/dev/null 2>&1 && TO="gtimeout 5"
  if prs=$($TO gh pr list --limit 10 --json title 2>/dev/null) && [ -n "$prs" ]; then
    recent_titles_json=$(printf '%s' "$prs" | jq -c '[.[].title]' 2>/dev/null || echo '[]')
  fi
fi

# ── release tooling ─────────────────────────────────────────────────────────
tooling=()
[ -f "$root/.github/workflows/release.yml" ] && tooling+=("release.yml-workflow")
{ [ -f "$root/release-please-config.json" ] || [ -f "$root/.release-please-manifest.json" ]; } && tooling+=("release-please")
{ ls "$root"/.releaserc* >/dev/null 2>&1 || grep -q '"semantic-release"' "$root/package.json" 2>/dev/null; } && tooling+=("semantic-release")
tooling_json=$(printf '%s\n' "${tooling[@]:-}" | grep . | jq -R . | jq -sc . || echo '[]')
version_commit=null; printf '%s\n' "$subjects" | grep -qE '^chore\(release\): v' && version_commit='"chore(release): vX"'
changelog=null; [ -f "$root/CHANGELOG.md" ] && changelog='"CHANGELOG.md"'

# ── issues ──────────────────────────────────────────────────────────────────
bug_template=null
for b in "$root"/.github/ISSUE_TEMPLATE/*bug* "$root"/.github/ISSUE_TEMPLATE/*Bug*; do
  [ -f "$b" ] && { bug_template="\"${b#"$root"/}\""; break; }
done

# ── prose (cache layer resolves prose_pending vs distilled) ─────────────────
prose_pending_json='[]'
[ -f "$root/CONTRIBUTING.md" ] && prose_pending_json='["CONTRIBUTING.md"]'

jq -n \
  --arg root "$root" --arg gen "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg sh "$src_hash" \
  --arg conv "$convention" --argjson types "$types_json" --arg scope "$scope" \
  --argjson pr_suffix "$pr_suffix" --argjson samples "$samples_json" \
  --arg bpat "$bpattern" --argjson bpre "$bprefixes_json" --argjson bex "$bexamples_json" \
  --argjson prtpl "$pr_template" --argjson prsec "$pr_sections_json" \
  --argjson ghavail "$gh_available" --argjson titles "$recent_titles_json" \
  --argjson tooling "$tooling_json" --argjson vcommit "$version_commit" --argjson chlog "$changelog" \
  --argjson bug "$bug_template" --argjson prose "$prose_pending_json" \
  '{
    schema_version: 1, repo_root: $root, generated_at: $gen, source_hash: $sh,
    commit_format: { convention: $conv, types: $types, scope: $scope, pr_suffix: $pr_suffix, samples: $samples },
    branch_naming: { pattern: $bpat, prefixes: $bpre, examples: $bex },
    pr: { template_path: $prtpl, title_format: $conv, body_sections: $prsec, recent_titles: $titles },
    release: { tooling: $tooling, version_commit: $vcommit, changelog: $chlog },
    issues: { bug_template_path: $bug, required_fields: [] },
    prose_pending: $prose, prose_distilled: {},
    gh_available: $ghavail
  }'
