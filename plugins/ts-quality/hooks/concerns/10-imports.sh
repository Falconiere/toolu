# --- Existing checks ---

# The "../ import → @/ alias" guidance is only actionable when the project
# actually defines a `@/*` path alias. In a repo that uses plain NodeNext
# relative imports with no alias configured, the recommendation cannot be
# followed, so flagging `../` there is a false positive that blocks every edit
# to such a file. Gate the check on the alias existing: walk up from the edited
# file to PROJECT_ROOT for a tsconfig whose compilerOptions.paths declares a
# `@/`-prefixed key. Tolerant of a missing / JSONC / oddly-shaped tsconfig (jq
# failure → treat as "no alias", so we never emit advice the project can't act
# on).
_ts_has_at_alias() {
  local dir root cfg
  dir="$(cd "$(dirname "$FILE_PATH")" 2>/dev/null && pwd)" || return 1
  root="$(cd "${PROJECT_ROOT:-/}" 2>/dev/null && pwd)" || root=""
  while [ -n "$dir" ]; do
    for cfg in "$dir/tsconfig.json" "$dir/tsconfig.base.json"; do
      [ -f "$cfg" ] || continue
      if jq -e '[(.compilerOptions.paths // {}) | keys[] | select(startswith("@/"))] | length > 0' "$cfg" >/dev/null 2>&1; then
        return 0
      fi
    done
    [ "$dir" = "$root" ] && break
    [ "$dir" = "/" ] && break
    dir="$(dirname "$dir")"
  done
  return 1
}

if grep -q 'from ["'"'"']\.\./' "$FILE_PATH" 2>/dev/null && _ts_has_at_alias; then
  add_error "Forbidden ../ import in $FILE_PATH — use @/ alias"
fi

