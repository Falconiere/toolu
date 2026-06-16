#!/usr/bin/env bash
# tooling/release.sh [--dry-run] [--no-notes] <major.minor.patch>
#
# Atomic version bump for the toolu marketplace. The auto-update gate only
# re-extracts a plugin when its plugin.json "version" changes, so a release that
# forgets to bump a changed plugin silently ships stale code to users. This
# script removes that manual step:
#
#   - package.json + the toolu plugin  -> <new-version>  (toolu anchors the
#     monorepo; release.yml's tag gate checks plugins/toolu against the git tag).
#   - every OTHER plugin whose files changed since the last v*.*.* tag -> a PATCH
#     bump of its own (independent) version. Unchanged plugins are left alone.
#   - docs/releases/v<new-version>.md  -> auto-drafted template skeleton with
#     per-plugin (old -> new) attribution. Refuses to overwrite an existing
#     file (pass --no-notes to skip).
#
# --dry-run / --plan prints the full bump plan and writes nothing.
#
# Versions are edited in place by line (NOT via jq, which would reorder keys and
# restyle the manifests). RELEASE_ROOT overrides the repo root for tests.
set -euo pipefail

ROOT="${RELEASE_ROOT:-$(cd "${BASH_SOURCE%/*}/.." && pwd)}"
cd "$ROOT"

usage() {
  cat <<'EOF'
usage: release.sh [--dry-run] [--no-notes] <major.minor.patch>
       release.sh --help
EOF
}

# Strict major.minor.patch (no pre-release/build/extra segments) — the bumper
# and the manifests assume exactly three numeric components.
is_semver() { [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; }

# Argument parsing. Flags before the version arg; --help short-circuits.
DRY_RUN=0
DRAFT_NOTES=1
NEW=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run|--plan) DRY_RUN=1 ;;
    --no-notes)       DRAFT_NOTES=0 ;;
    -h|--help)        usage; exit 0 ;;
    --*)              printf 'release.sh: unknown flag %s\n' "$1" >&2; usage >&2; exit 2 ;;
    *)                NEW="$1" ;;
  esac
  shift
done
is_semver "$NEW" || { usage >&2; exit 1; }

# Pre-flight: refuse to clobber an existing notes file. Done as early as
# possible (right after arg parsing, before any git work) so apply-mode users
# fail fast and don't pay for the diffstat/plan computation. Skipped in
# --dry-run mode so the plan still prints for review.
NOTES_PATH="docs/releases/v${NEW}.md"
if [ "$DRAFT_NOTES" -eq 1 ] && [ "$DRY_RUN" -eq 0 ] && [ -e "$NOTES_PATH" ]; then
  printf 'release.sh: %s already exists — refusing to overwrite (remove it or pass --no-notes)\n' "$NOTES_PATH" >&2
  exit 1
fi

# Replace the "version" field of a JSON file in place, touching only that line.
set_version() {  # $1=file  $2=version
  local f="$1" v="$2"
  is_semver "$v" || { printf 'refusing to write non-semver version "%s" to %s\n' "$v" "$f" >&2; return 1; }
  grep -qE '^[[:space:]]*"version":' "$f" || { printf 'no version field in %s\n' "$f" >&2; return 1; }
  sed -E "s|^([[:space:]]*\"version\":[[:space:]]*\")[^\"]*(\")|\1${v}\2|" "$f" > "$f.tmp"
  mv "$f.tmp" "$f"
}

bump_patch() {  # $1=x.y.z -> x.y.(z+1); fails loudly on a malformed input
  local a b c IFS=.
  is_semver "$1" || { printf 'refusing to bump non-semver version "%s"\n' "$1" >&2; return 1; }
  read -r a b c <<<"$1"
  printf '%s.%s.%s' "$a" "$b" "$((c + 1))"
}

cur_version() {  # $1=manifest -> its "version" value (may be empty/malformed)
  grep -E '^[[:space:]]*"version":' "$1" | head -1 | sed -E 's|.*"version":[[:space:]]*"([^"]*)".*|\1|'
}

# Per-plugin diff stat against the last release tag, for the plan banner.
# Falls back to "(no previous tag)" when there's no v*.*.* baseline.
diffstat_for() {  # $1=path (file or dir)
  if [ -z "$last_tag" ]; then
    printf '(no previous tag)'
  else
    local s
    s="$(git diff --shortstat "$last_tag" -- "$1" 2>/dev/null || true)"
    if [ -z "$s" ]; then
      printf '(no diff)'
    else
      printf '%s' "$s"
    fi
  fi
}

last_tag="$(git describe --tags --abbrev=0 --match 'v*.*.*' 2>/dev/null || true)"
[ -n "$last_tag" ] || printf 'no v*.*.* tag found — bumping every changed-or-not plugin\n' >&2

# Pass 1 — resolve every change and validate, writing NOTHING yet, so a single
# malformed manifest aborts the release before any file is touched (atomicity).
# Parallel arrays track (file, old, new, stat, plugin-name-or-empty) so the
# notes draft can reproduce the same per-plugin old->new attribution.
declare -a files=() old_versions=() new_versions=() stats=() plugin_names=()
declare -a skipped=()

# Anchor lines: package.json + toolu (toolu anchors the monorepo).
files+=(package.json)
old_versions+=("$(cur_version package.json)")
new_versions+=("$NEW")
stats+=("")
plugin_names+=("")

files+=(plugins/toolu/.claude-plugin/plugin.json)
toolu_old="$(cur_version plugins/toolu/.claude-plugin/plugin.json)"
old_versions+=("$toolu_old")
new_versions+=("$NEW")
stats+=("$(diffstat_for plugins/toolu)")
plugin_names+=("toolu")

for manifest in plugins/*/.claude-plugin/plugin.json; do
  name="$(basename "$(dirname "$(dirname "$manifest")")")"
  [ "$name" = toolu ] && continue
  dir="plugins/$name"
  # Skip plugins untouched since the last release (only when we have a baseline).
  if [ -n "$last_tag" ] && git diff --quiet "$last_tag" -- "$dir"; then
    skipped+=("$name")
    continue
  fi
  cur="$(cur_version "$manifest")"
  next="$(bump_patch "$cur")" || { printf 'release.sh: %s has a malformed version "%s" — fix it first\n' "$manifest" "$cur" >&2; exit 1; }
  files+=("$manifest")
  old_versions+=("$cur")
  new_versions+=("$next")
  stats+=("$(diffstat_for "$dir")")
  plugin_names+=("$name")
done

# Always print the plan so apply and dry-run look the same up to the "applied"
# line. Greppable via the `release.sh:` prefix.
print_plan() {
  if [ "$DRY_RUN" -eq 1 ]; then
    printf 'release.sh: plan for v%s (dry-run; nothing will be written)\n' "$NEW"
  else
    printf 'release.sh: plan for v%s\n' "$NEW"
  fi
  local i path old new s
  for i in "${!files[@]}"; do
    path="${files[$i]}"; old="${old_versions[$i]}"; new="${new_versions[$i]}"; s="${stats[$i]}"
    if [ -n "$s" ]; then
      printf 'release.sh:   %-55s %s -> %s   %s\n' "$path" "$old" "$new" "$s"
    else
      printf 'release.sh:   %-55s %s -> %s\n' "$path" "$old" "$new"
    fi
  done
  if [ "${#skipped[@]}" -gt 0 ]; then
    printf 'release.sh: unchanged plugins (skipped): %s\n' "$(IFS=,; echo "${skipped[*]}")"
  fi
  if [ "$DRY_RUN" -eq 0 ] && [ "$DRAFT_NOTES" -eq 1 ]; then
    if [ -n "$last_tag" ]; then
      printf 'release.sh: will draft %s (template skeleton; edit the TODO lines before committing)\n' "$NOTES_PATH"
    else
      printf 'release.sh: will draft %s (initial release; template skeleton)\n' "$NOTES_PATH"
    fi
  fi
}
print_plan

# Dry-run: stop here. No files written, no notes drafted.
if [ "$DRY_RUN" -eq 1 ]; then
  printf 'release.sh: re-run without --dry-run to apply.\n'
  exit 0
fi

# Pass 2 — apply. set_version re-validates each value defensively.
for i in "${!files[@]}"; do
  set_version "${files[$i]}" "${new_versions[$i]}"
done

# Pass 3 — draft the release notes (skipped with --no-notes). Layout matches
# the v1.10.0 / v1.12.0 shape: title, release date, Highlights placeholder,
# per-plugin bullets with (old -> new). The human fills in the TODO lines
# before committing; release.yml uploads the resulting file as the GitHub
# Release body via --notes-file.
if [ "$DRAFT_NOTES" -eq 1 ]; then
  mkdir -p "$(dirname "$NOTES_PATH")"
  today="$(date +%Y-%m-%d)"
  {
    printf '# toolu v%s\n\nReleased: %s\n\n' "$NEW" "$today"
    printf '## Highlights\n\n<!-- TODO: write 2-3 sentence summary of the release. -->\n\n'
    # Always draft the Upgrade notes section so the template and the script
    # output match. The human deletes the whole block (header + TODO) if there
    # are no user-facing steps (e.g. /plugin install ...).
    # shellcheck disable=SC2016  # backticks are literal markdown code spans in the output
    printf '## Upgrade notes\n\n<!-- TODO: write any user-facing upgrade steps (e.g. `/plugin install foo@toolu`).\n     Delete this entire section if there are none. -->\n\n'
    if [ -n "$last_tag" ]; then
      printf '## Included changes since %s\n\n' "$last_tag"
    else
      printf '## Included changes\n\n<!-- TODO: this is the initial release; list the major surfaces shipped. -->\n\n'
    fi
    # Per-plugin bullets (skip the package.json anchor, emit toolu last so the
    # "headline" plugin closes the list).
    i=0
    while [ "$i" -lt "${#files[@]}" ]; do
      n="${plugin_names[$i]}"
      if [ -n "$n" ] && [ "$n" != toolu ]; then
        # shellcheck disable=SC2016  # backticks are literal markdown code spans in the output
        printf -- '- `%s`: <!-- TODO one-line summary --> (`%s` %s -> %s)\n' \
          "$n" "$n" "${old_versions[$i]}" "${new_versions[$i]}"
      fi
      i=$((i+1))
    done
    # Anchor: toolu was set to NEW above; toolu_old captured the pre-bump.
    # shellcheck disable=SC2016  # backticks are literal markdown code spans in the output
    printf -- '- `toolu`: <!-- TODO one-line summary --> (`toolu` %s -> %s)\n' \
      "$toolu_old" "$NEW"
    unset i n
  } > "$NOTES_PATH"
  printf 'release.sh: drafted %s\n' "$NOTES_PATH"
fi

printf 'release.sh: applied v%s\n' "$NEW"
