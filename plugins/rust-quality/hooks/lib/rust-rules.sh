#!/usr/bin/env bash
# Canonical Rust rule library — the single source of truth shared by the
# per-edit gate (concern wrappers concatenate this file) and the repo-wide
# scanner (rust-scan.sh sources this file). Same bytes, two consumers.
#
# Every rule_* function is PURE: it takes a file path (or crate root) and
# ECHOES zero or more structured violation records, one per line, TAB-separated:
#
#     <rule>\t<severity>\t<file>\t<line>\t<autofix>\t<message>
#
#   rule     = file-size | mod-rs-no-logic | generic-name | test-location
#              | module-doc | fn-size | impl-size | layering
#   severity = block | advisory
#   autofix  = split | rename | move | restructure | manual | clippy-fix | fmt
#
# Empty output = clean. NO exit, NO gate writes, NO add_error — callers decide.
#
# Reuse (sourced by the CALLER, assumed present at runtime — NOT re-sourced
# here): detect.sh provides count_code_lines, has_unterminated_block, read_list;
# quality-config.sh provides quality_threshold, toolu_load_config, $TOOLU_CFG_JSON.
#
# This file must stay shellcheck-clean.

# A literal tab, used as the record field separator. Kept in a variable so the
# field layout is unmistakable and a stray space can never sneak in.
RR_TAB=$(printf '\t')

# rr_emit RULE SEVERITY FILE LINE AUTOFIX MESSAGE
# Print one violation record. The message is last; both tabs AND newlines in it
# would corrupt the tab-separated, one-record-per-line format, so both are
# squeezed to spaces defensively (a message is meant to be a single line).
rr_emit() {
  local rule="$1" sev="$2" file="$3" line="$4" autofix="$5" msg="$6"
  msg=${msg//$RR_TAB/ }
  msg=${msg//$'\n'/ }
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$rule" "$sev" "$file" "$line" "$autofix" "$msg"
}

# ---------------------------------------------------------------------------
# Config resolvers (quality_threshold is int-only, so bools/lists need their own)
# ---------------------------------------------------------------------------

# rr_config_bool KEY DEFAULT(0|1) -> echoes "1" or "0".
# Reads .lang.rust.<key> from $TOOLU_CFG_JSON. Accepts true/false/1/0. Degrades
# to DEFAULT when jq or the config is unavailable.
rr_config_bool() {
  local key="$1" def="$2" v
  command -v toolu_load_config >/dev/null 2>&1 && toolu_load_config 2>/dev/null
  if command -v jq >/dev/null 2>&1 && [ -n "${TOOLU_CFG_JSON:-}" ]; then
    v=$(jq -r --arg k "$key" '
      ((.lang? // {}).rust? // {})[$k]?
      | if . == true or . == 1 or . == "true" or . == "1" then "1"
        elif . == false or . == 0 or . == "false" or . == "0" then "0"
        else empty end
    ' <<< "$TOOLU_CFG_JSON" 2>/dev/null)
    [ -n "$v" ] && { printf '%s' "$v"; return 0; }
  fi
  printf '%s' "$def"
}

# rr_config_list KEY DEFAULT_CSV -> echoes a comma-separated list.
# Reads .lang.rust.<key> from $TOOLU_CFG_JSON. Accepts a JSON array or a csv
# string. Degrades to DEFAULT_CSV when jq or the config is unavailable.
rr_config_list() {
  local key="$1" def="$2" v
  command -v toolu_load_config >/dev/null 2>&1 && toolu_load_config 2>/dev/null
  if command -v jq >/dev/null 2>&1 && [ -n "${TOOLU_CFG_JSON:-}" ]; then
    v=$(jq -r --arg k "$key" '
      ((.lang? // {}).rust? // {})[$k]?
      | if type == "array" then (map(tostring) | join(","))
        elif type == "string" and (. | length) > 0 then .
        else empty end
    ' <<< "$TOOLU_CFG_JSON" 2>/dev/null)
    [ -n "$v" ] && { printf '%s' "$v"; return 0; }
  fi
  printf '%s' "$def"
}

# ---------------------------------------------------------------------------
# Crate resolution
# ---------------------------------------------------------------------------

# nearest_cargo_toml FILE -> echoes the path of the nearest enclosing
# Cargo.toml, walking up from the file's directory. Empty if none is found.
nearest_cargo_toml() {
  local file="$1" dir
  [ -n "$file" ] || return 0
  if [ -d "$file" ]; then dir="$file"; else dir=$(dirname "$file"); fi
  # Resolve to an absolute path so the upward walk terminates at "/".
  dir=$(cd "$dir" 2>/dev/null && pwd) || return 0
  while [ -n "$dir" ]; do
    if [ -f "$dir/Cargo.toml" ]; then
      printf '%s' "$dir/Cargo.toml"
      return 0
    fi
    [ "$dir" = "/" ] && break
    dir=$(dirname "$dir")
  done
  return 0
}

# ---------------------------------------------------------------------------
# rule_file_size — code lines > maxFileLines (default 250)
# ---------------------------------------------------------------------------
rule_file_size() {
  local file="$1" max count approx="" msg
  [ -f "$file" ] || return 0
  max=$(rust_max_file_lines)
  count=$(count_code_lines "$file")
  [ -n "$count" ] || return 0
  if [ "$count" -gt "$max" ]; then
    if has_unterminated_block "$file"; then
      approx=" (size approximated — an unterminated /* or a string containing /* may be affecting the count)"
    fi
    msg="File exceeds ${max}-line limit (${count} code lines, blanks/comments excluded)${approx} — split into submodules"
    rr_emit file-size block "$file" 1 split "$msg"
  fi
}

# ---------------------------------------------------------------------------
# rule_mod_rs_no_logic — a mod.rs must contain only mod/pub use/doc/attributes
# ---------------------------------------------------------------------------
rule_mod_rs_no_logic() {
  local file="$1" base
  [ -f "$file" ] || return 0
  base=$(basename "$file")
  [ "$base" = "mod.rs" ] || return 0
  [ "$(rr_config_bool modRsNoLogic 1)" = "1" ] || return 0
  # First offending line: anything that is not a `mod …;`, `pub use …;`, a
  # `//!`/`//` comment, a blank line, or an attribute. Block comments are
  # stripped first so a doc-comment block can't false-positive. Any
  # fn/struct/enum/impl/const/static/let/type token (as a leading item) trips it.
  local offending
  offending=$(awk '
    { line=$0
      if (inblock) {
        idx=index(line,"*/")
        if (idx>0) { line=substr(line, idx+2); inblock=0 } else next
      }
      while ((s=index(line,"/*"))>0) {
        rest=substr(line, s+2); e=index(rest,"*/")
        if (e>0) { line=substr(line,1,s-1) substr(rest, e+2) }
        else { line=substr(line,1,s-1); inblock=1; break }
      }
      stripped=line
      sub(/^[ \t]+/, "", stripped); sub(/[ \t]+$/, "", stripped)
      if (stripped == "") next
      if (stripped ~ /^\/\//) next
      if (stripped ~ /^#!?\[/) next
      if (stripped ~ /^(pub(\([^)]*\))?[ \t]+)?mod[ \t]/) next
      if (stripped ~ /^pub([ \t]+|\([^)]*\)[ \t]+)use[ \t]/) next
      if (stripped ~ /^use[ \t]/) next
      print NR; exit
    }
  ' "$file" 2>/dev/null)
  if [ -n "$offending" ]; then
    rr_emit mod-rs-no-logic block "$file" "$offending" restructure \
      "mod.rs contains logic — keep it to mod declarations and pub use re-exports; move code into a named submodule"
  fi
}

# ---------------------------------------------------------------------------
# rule_generic_name — basename in the forbidden list; shared/ & common/ exempt
# ---------------------------------------------------------------------------
rule_generic_name() {
  local file="$1" base stem parent forbidden IFS
  [ -f "$file" ] || return 0
  base=$(basename "$file")
  case "$base" in *.rs) ;; *) return 0 ;; esac
  stem=${base%.rs}
  # Exempt when the file's parent directory is shared/ or common/.
  parent=$(basename "$(dirname "$file")")
  if [ "$parent" = "shared" ] || [ "$parent" = "common" ]; then
    return 0
  fi
  forbidden=$(rr_config_list forbiddenFileNames "utils,helpers,common,misc")
  local name matched=0
  IFS=','
  for name in $forbidden; do
    name=${name# }; name=${name% }
    [ -n "$name" ] || continue
    if [ "$stem" = "$name" ]; then matched=1; break; fi
  done
  unset IFS
  if [ "$matched" = "1" ]; then
    rr_emit generic-name block "$file" 1 rename \
      "Generic file name '${base}' — rename to a name that describes its single responsibility"
  fi
}

# ---------------------------------------------------------------------------
# rule_test_location — port of 20-tests.sh
# ---------------------------------------------------------------------------
rule_test_location() {
  local file="$1"
  [ -f "$file" ] || return 0
  local has_inline_cfg=0
  if [[ "$file" == */src/* ]] \
     && grep -qE '^[[:space:]]*#\[cfg\((test\)|all\(test\b|any\(test\b)' "$file" 2>/dev/null; then
    rr_emit test-location block "$file" 1 move \
      "Inline #[cfg(test)] in src/ — extract tests into the sibling tests/ directory"
    has_inline_cfg=1
  fi

  local is_test=0
  case "$(basename "$file")" in
    *_test.rs|*_tests.rs) is_test=1 ;;
  esac
  if [ "$is_test" -eq 0 ] \
     && grep -qE '^[[:space:]]*#\[([A-Za-z_][A-Za-z0-9_]*::)*(test|test_case)\b|^[[:space:]]*#\[rstest\b' "$file" 2>/dev/null; then
    is_test=1
  fi

  if [ "$is_test" -eq 1 ] && [ "$has_inline_cfg" -eq 0 ]; then
    if [[ "$file" != */tests/* ]]; then
      rr_emit test-location block "$file" 1 move \
        "Rust test file outside tests/ — move it to a sibling tests/ directory"
    else
      local after subdir
      after="${file##*/tests/}"
      if [[ "$after" == */* ]]; then
        subdir="${after%%/*}"
        if [ "$subdir" != "fixtures" ] && [ "$subdir" != "helpers" ] && [ "$subdir" != "common" ]; then
          rr_emit test-location block "$file" 1 move \
            "Rust test nested in tests/${subdir}/ — keep tests/ flat (only fixtures/helpers/common subdirs allowed)"
        fi
      fi
    fi
  fi
}

# ---------------------------------------------------------------------------
# rule_module_doc — //! header + (case-insensitive) # Public API / # Usage
# ---------------------------------------------------------------------------
# Whether the file is a Rust test file (mirrors rule_test_location's detection).
rr_is_test_file() {
  local file="$1"
  case "$(basename "$file")" in
    *_test.rs|*_tests.rs) return 0 ;;
  esac
  grep -qE '^[[:space:]]*#\[([A-Za-z_][A-Za-z0-9_]*::)*(test|test_case)\b|^[[:space:]]*#\[rstest\b|^[[:space:]]*#\[cfg\((test\)|all\(test\b|any\(test\b)' "$file" 2>/dev/null
}

rule_module_doc() {
  local file="$1" base
  [ -f "$file" ] || return 0
  base=$(basename "$file")
  [ "$(rr_config_bool requireModuleDoc 1)" = "1" ] || return 0

  # Applies to mod.rs / lib.rs / main.rs, and to any src/ non-test .rs file.
  local applies=0
  case "$base" in
    mod.rs|lib.rs|main.rs) applies=1 ;;
  esac
  if [ "$applies" -eq 0 ] && [[ "$file" == */src/* ]]; then
    case "$base" in
      *.rs) rr_is_test_file "$file" || applies=1 ;;
    esac
  fi
  [ "$applies" -eq 1 ] || return 0

  # Is there a //! header anywhere before the first item? Scan leading lines,
  # skipping blanks, // line comments, and attributes; the first //! found wins,
  # and the first real item ends the scan.
  local has_header
  has_header=$(awk '
    { line=$0; sub(/^[ \t]+/, "", line)
      if (line == "") next
      if (line ~ /^\/\/!/) { print "1"; exit }
      if (line ~ /^\/\//) next
      if (line ~ /^#!\[/) next
      print "0"; exit
    }
    END { }
  ' "$file" 2>/dev/null)

  if [ "$has_header" != "1" ]; then
    rr_emit module-doc block "$file" 1 manual \
      "Missing module-level //! doc header — add a concise //! summarising the module"
    return 0
  fi

  # Header present — check for a # Public API or # Usage section (case-insensitive
  # substring on any //! line).
  if ! grep -iqE '^[[:space:]]*//!.*#[[:space:]]*(public api|usage)' "$file" 2>/dev/null; then
    rr_emit module-doc advisory "$file" 1 manual \
      "Module //! doc lacks a '# Public API' or '# Usage' section — document the module's surface"
  fi
}

# ---------------------------------------------------------------------------
# rule_fn_size — port of 50-size-fn.sh awk brace-depth; > maxFnLines (default 80)
# ---------------------------------------------------------------------------
rule_fn_size() {
  local file="$1" max hits
  [ -f "$file" ] || return 0
  max=$(rust_max_fn_lines)
  hits=$(awk -v max="$max" -v q="'" '
    !infn && /^[[:space:]]*(pub(\([^)]+\))?[[:space:]]+)?((async|const|unsafe|extern)([[:space:]]+"[^"]*")?[[:space:]]+)*fn / {
      infn=1; start=NR; depth=0; opened=0
    }
    infn {
      line=$0
      gsub(/\\"/, "", line)
      gsub(/"[^"]*"/, "", line)
      gsub(q "[{}]" q, "", line)
      no=gsub(/\{/, "{", line); nc=gsub(/\}/, "}", line)
      depth += no - nc
      if (no > 0) opened=1
      if (opened && depth <= 0) {
        len=NR-start
        if (len > max) printf "%d %d\n", start, len
        infn=0
      }
      if (!opened && $0 ~ /;[[:space:]]*$/) infn=0
    }
  ' "$file" 2>/dev/null)
  local line len
  while read -r line len; do
    [ -n "$line" ] || continue
    rr_emit fn-size block "$file" "$line" split \
      "Function exceeds ${max}-line limit (${len} lines) — extract helpers"
  done <<< "$hits"
}

# ---------------------------------------------------------------------------
# rule_impl_size — port of 55-size-impl.sh; > maxImplLines (default 200)
# ---------------------------------------------------------------------------
rule_impl_size() {
  local file="$1" max hits
  [ -f "$file" ] || return 0
  max=$(rust_max_impl_lines)
  hits=$(awk -v max="$max" -v q="'" '
    !inimpl && /^[[:space:]]*(unsafe[[:space:]]+)?impl[[:space:]<]/ {
      inimpl=1; start=NR; depth=0; opened=0
    }
    inimpl {
      line=$0
      gsub(/\\"/, "", line)
      gsub(/"[^"]*"/, "", line)
      gsub(q "[{}]" q, "", line)
      no=gsub(/\{/, "{", line); nc=gsub(/\}/, "}", line)
      depth += no - nc
      if (no > 0) opened=1
      if (opened && depth <= 0) {
        len=NR-start
        if (len > max) printf "%d %d\n", start, len
        inimpl=0
      }
    }
  ' "$file" 2>/dev/null)
  local line len
  while read -r line len; do
    [ -n "$line" ] || continue
    rr_emit impl-size block "$file" "$line" split \
      "Impl block exceeds ${max}-line limit (${len} lines) — split into trait impls or modules"
  done <<< "$hits"
}

# ---------------------------------------------------------------------------
# rule_layering_file — per-file layering heuristic (see rust-rules-layering.sh)
# ---------------------------------------------------------------------------
# Sourced or concatenated alongside this file. The gate's register.sh
# concatenates both; rust-scan.sh sources both. Keep them adjacent.
_RR_SELF_DIR="${BASH_SOURCE%/*}"
if ! declare -f rule_layering_file >/dev/null 2>&1; then
  if [ -f "$_RR_SELF_DIR/rust-rules-layering.sh" ]; then
    # shellcheck source=./rust-rules-layering.sh
    . "$_RR_SELF_DIR/rust-rules-layering.sh"
  fi
fi
