#!/usr/bin/env bash
# conventions-cache — cache/serve the convention profile + persist distilled
# prose. Recomputes (via conventions-detect.sh) only when the declared-file
# source_hash changed, the cache is >7 days old, or --refresh/--save-prose.
# Usage: conventions-cache.sh [--json] [--refresh] [--save-prose <file>]
set -eu

LIBDIR="$(cd "$(dirname "$0")" && pwd)"
DETECT="$LIBDIR/conventions-detect.sh"
root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
command -v jq >/dev/null 2>&1 || { echo '{"error":"jq missing"}'; exit 0; }

_sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi; }

mode="summary"; refresh=0; save_file=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --json) mode="json" ;;
    --refresh) refresh=1 ;;
    --save-prose) save_file="${2:-}"; shift ;;
    *) echo "gb conventions: unknown arg '$1'" >&2; exit 1 ;;
  esac
  shift
done

cfg="${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}"
cache_dir="$cfg/toolu/git-better/conventions"
repo_id="$(printf '%s' "$root" | _sha256)"
cache_file="$cache_dir/$repo_id.json"

emit() {  # $1 = profile JSON
  if [ "$mode" = "json" ]; then
    printf '%s\n' "$1"
  else
    printf '%s' "$1" | jq -r '
      "commit:  \(.commit_format.convention) | scope \(.commit_format.scope) | suffix \(.commit_format.pr_suffix // "none")",
      "branch:  \(.branch_naming.pattern) \(.branch_naming.prefixes)",
      "pr:      template \(.pr.template_path // "none") | sections \(.pr.body_sections)",
      "release: \(.release.tooling) | \(.release.version_commit // "none")",
      "prose:   pending \(.prose_pending)"'
  fi
}

# ── fast path: fresh cache, no write requested ──────────────────────────────
if [ -f "$cache_file" ] && [ "$refresh" = 0 ] && [ -z "$save_file" ]; then
  cached_hash="$(jq -r '.source_hash // ""' "$cache_file" 2>/dev/null || echo "")"
  cur_hash="$(bash "$DETECT" --source-hash "$root")"
  aged="$(find "$cache_file" -mtime +7 2>/dev/null || true)"
  if [ "$cached_hash" = "$cur_hash" ] && [ -z "$aged" ]; then
    emit "$(cat "$cache_file")"
    exit 0
  fi
fi

# ── recompute, merging prior distilled prose ────────────────────────────────
new="$(bash "$DETECT" "$root")"
prior_prose='{}'
[ -f "$cache_file" ] && prior_prose="$(jq -c '.prose_distilled // {}' "$cache_file" 2>/dev/null || echo '{}')"

if [ -n "$save_file" ]; then
  [ -f "$root/$save_file" ] || { echo "gb conventions: no such file '$save_file'" >&2; exit 1; }
  rules="$(cat)"   # distilled text on STDIN
  [ -n "$rules" ] || { echo "gb conventions: --save-prose needs distilled text on STDIN" >&2; exit 1; }
  fhash="$(_sha256 < "$root/$save_file")"
  prior_prose="$(printf '%s' "$prior_prose" | jq --arg f "$save_file" --arg h "$fhash" --arg r "$rules" '.[$f]={hash:$h,rules:$r}')"
fi

# recompute prose_pending: present prose file whose current hash != distilled hash
pending=()
if [ -f "$root/CONTRIBUTING.md" ]; then
  ch="$(_sha256 < "$root/CONTRIBUTING.md")"
  dh="$(printf '%s' "$prior_prose" | jq -r '.["CONTRIBUTING.md"].hash // ""')"
  [ "$ch" = "$dh" ] || pending+=("CONTRIBUTING.md")
fi
pending_json="$(printf '%s\n' "${pending[@]:-}" | grep . | jq -R . | jq -sc . || echo '[]')"

merged="$(printf '%s' "$new" | jq --argjson pd "$prior_prose" --argjson pp "$pending_json" '.prose_distilled=$pd | .prose_pending=$pp')"

mkdir -p "$cache_dir" 2>/dev/null || true
tmp="$cache_file.tmp.$$"
if printf '%s\n' "$merged" > "$tmp" 2>/dev/null; then
  mv "$tmp" "$cache_file" 2>/dev/null || rm -f "$tmp"
fi

emit "$merged"
