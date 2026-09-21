# lint-suppressions.sh — dead code must be removed or wired, never hidden.
#
# The compiler/linter owns finding dead declarations. This check owns the
# independent fact that source code must not turn that enforcement off:
#
# - a blanket eslint/oxlint disable can silence no-unused-vars along with every
#   other rule;
# - a scoped no-unused-vars disable hides exactly the dead code this convention
#   exists to expose;
# - Rust's allow/warn/expect attributes can lower or consume `dead_code`
#   directly or through the `unused` lint group, overriding Cargo's deny level.
#
# Script and Rust paths are scanned separately. An oxlint directive in Rust is
# just prose, and a Rust attribute-like string in TypeScript is not an
# attribute; applying all patterns to all languages would invent violations.

GR_LS_LINE_BLANKET='//[[:space:]]*(oxlint|eslint)-disable(-next-line|-line)?[[:space:]]*(--[^[:cntrl:]]*)?$'
GR_LS_LINE_UNUSED='//[[:space:]]*(oxlint|eslint)-disable(-next-line|-line)?[[:space:]]+([^,[:space:]]+[[:space:]]*,[[:space:]]*)*((eslint|typescript|@typescript-eslint)/)?no-unused-vars([[:space:],]|$)'
GR_LS_BLOCK_START='/\*[[:space:]]*(oxlint|eslint)-disable'
GR_LS_BLOCK_BLANKET='/\*[[:space:]]*(oxlint|eslint)-disable(-next-line|-line)?[[:space:]]*(--[^*]*)?\*/'
GR_LS_BLOCK_UNUSED='/\*[[:space:]]*(oxlint|eslint)-disable(-next-line|-line)?[[:space:]]+([^,[:space:]*]+[[:space:]]*,[[:space:]]*)*((eslint|typescript|@typescript-eslint)/)?no-unused-vars([[:space:],*]|$)'
GR_LS_RUST_DIRECT='^#!?\[(r#)?(allow|warn|expect)\(([^,)]*,)*(r#)?(dead_code|unused)(,[^)]*)?\)\]'
GR_LS_RUST_CFG_ATTR='^#!?\[(r#)?cfg_attr\(.*,(r#)?(allow|warn|expect)\(([^,)]*,)*(r#)?(dead_code|unused)(,[^)]*)?\).*\)\]'
GR_LS_SCRIPT_CANDIDATE='(oxlint|eslint)-disable'
GR_LS_RUST_CANDIDATE='(^|[^[:alnum:]_])(allow|warn|expect)([^[:alnum:]_]|$)'

gr_ls_load_syntax() {
  local helper required
  [ "${GR_LS_SYNTAX_LOADED-0}" -eq 1 ] && return 0
  helper="$GR_DIR/lib/lint-syntax.sh"
  [ -r "$helper" ] || gr_fatal "lint-suppressions syntax helper is missing or unreadable: $helper"
  . "$helper" || gr_fatal "lint-suppressions could not load syntax helper: $helper"
  for required in gr_ls_script_syntax_reset gr_ls_script_syntax_line \
    gr_ls_rust_syntax_reset gr_ls_rust_syntax_line; do
    [ "$(type -t "$required")" = function ] \
      || gr_fatal "lint-suppressions syntax helper is incomplete: missing $required"
  done
  GR_LS_SYNTAX_LOADED=1
}

gr_ls_block_forbidden() {
  [[ $1 =~ $GR_LS_BLOCK_BLANKET ]] || [[ $1 =~ $GR_LS_BLOCK_UNUSED ]]
}

# gr_ls_script_forbidden <path> — recognize line directives plus single- and
# multi-line block directives. A prose comment remains legal because the lint
# directive must be the first token after the actual comment delimiter.
gr_ls_script_forbidden() {
  local path source source_offset raw_length line block after in_block
  path=$1
  source=${2-}
  [ -n "$source" ] || source=$(< "$path") || return 2
  GR_LS_JS_SOURCE=$source
  GR_LS_JS_SOURCE_OFFSET=0
  source_offset=0
  block=''
  in_block=0
  gr_ls_script_syntax_reset "$path"
  while IFS= read -r line || [ -n "$line" ]; do
    raw_length=${#line}
    GR_LS_JS_SOURCE_OFFSET=$source_offset
    gr_ls_script_syntax_line "$line"
    source_offset=$((source_offset + raw_length + 1))
    line=$GR_LS_SANITIZED
    if [ "$in_block" -eq 1 ]; then
      block="$block $line"
      case "$line" in
        *'*/'*)
          gr_ls_block_forbidden "$block" && return 0
          block=''
          in_block=0
          ;;
      esac
      continue
    fi

    if [[ $line =~ $GR_LS_LINE_BLANKET ]] || [[ $line =~ $GR_LS_LINE_UNUSED ]]; then
      return 0
    fi
    if [[ $line =~ $GR_LS_BLOCK_START ]]; then
      block=$line
      after=${line#*'/*'}
      if [[ $after == *'*/'* ]]; then
        gr_ls_block_forbidden "$block" && return 0
        block=''
      else
        in_block=1
      fi
    fi
  done < "$path" || return 2
  return 1
}

gr_ls_rust_attr_forbidden() {
  local compact
  compact=${1//[[:space:]]/}
  [[ $compact =~ $GR_LS_RUST_DIRECT ]] || [[ $compact =~ $GR_LS_RUST_CFG_ATTR ]]
}

gr_ls_rust_bracket_depth() {
  local line depth i char
  line=$1
  depth=$2
  i=0
  while [ "$i" -lt "${#line}" ]; do
    char=${line:i:1}
    [ "$char" = '[' ] && depth=$((depth + 1))
    [ "$char" = ']' ] && depth=$((depth - 1))
    i=$((i + 1))
  done
  GR_LS_RUST_ATTR_DEPTH=$depth
}

# gr_ls_rust_forbidden <path> — collect an attribute through its closing
# bracket before matching. This covers rustfmt's one-line form and source that
# spreads allow/cfg_attr over several lines without treating prose as syntax.
gr_ls_rust_forbidden() {
  local path line trimmed attr in_attr depth
  path=$1
  attr=''
  in_attr=0
  depth=0
  gr_ls_rust_syntax_reset
  while IFS= read -r line || [ -n "$line" ]; do
    gr_ls_rust_syntax_line "$line"
    line=$GR_LS_SANITIZED
    if [ "$in_attr" -eq 1 ]; then
      attr="$attr $line"
      gr_ls_rust_bracket_depth "$line" "$depth"
      depth=$GR_LS_RUST_ATTR_DEPTH
      if [ "$depth" -le 0 ]; then
        gr_ls_rust_attr_forbidden "$attr" && return 0
        attr=''
        in_attr=0
        depth=0
      fi
      continue
    fi

    trimmed=${line#"${line%%[![:space:]]*}"}
    case "$trimmed" in
      '#['*|'#!['*)
        attr=$trimmed
        gr_ls_rust_bracket_depth "$trimmed" 0
        depth=$GR_LS_RUST_ATTR_DEPTH
        if [ "$depth" -le 0 ]; then
          gr_ls_rust_attr_forbidden "$attr" && return 0
          attr=''
          depth=0
        else
          in_attr=1
        fi
        ;;
    esac
  done < "$path" || return 2
  return 1
}

# gr_ls_scan <path> — file/hook mode. Read in Bash instead of launching grep:
# this path runs after every edit, and one process per check is measurable once
# workspace dispatch has already re-execed the package guardrail. Repo mode
# still uses one batched grep for the whole tree below.
gr_ls_scan() {
  local path kind status content
  path=$1
  case "$path" in
    *.ts|*.tsx|*.mts|*.cts|*.js|*.jsx|*.mjs|*.cjs|*.astro) kind=script ;;
    *.rs) kind=rust ;;
    *) return 0 ;;
  esac
  [ -f "$path" ] || return 0
  [ -r "$path" ] || gr_fatal "lint-suppressions cannot read $path"

  content=$(< "$path") || gr_fatal "lint-suppressions cannot read $path"
  if [ "$kind" = 'script' ]; then
    [[ $content =~ $GR_LS_SCRIPT_CANDIDATE ]] || return 0
  else
    [[ $content =~ $GR_LS_RUST_CANDIDATE ]] || return 0
  fi
  gr_ls_load_syntax

  if [ "$kind" = 'script' ]; then
    gr_ls_script_forbidden "$path" "$content"
  else
    gr_ls_rust_forbidden "$path"
  fi
  status=$?
  case "$status" in
    0)
      gr_violation lint-suppressions "$path" \
        'dead-code lint enforcement is disabled' \
        'delete the unused code or wire it into the program; do not suppress the lint'
      ;;
    1) ;;
    *) gr_fatal "lint-suppressions scan failed on $path" ;;
  esac
}

# gr_ls_repo_lists <script-list> <rust-list> — inventory live source files,
# including untracked edits, while pruning dependencies and generated output.
# Lists are NUL-delimited because filenames may contain spaces, colons, or
# newlines; the rest of guardrails already makes the same path-safety promise.
gr_ls_find_sources() {
  local list kind
  list=$1
  kind=$2
  if [ "$kind" = script ]; then
    find . \
      \( -path './.git' -o -path './node_modules' -o -path './dist' \
         -o -path './build' -o -path './out' -o -path './coverage' \
         -o -path './.wrangler' -o -path './.next' -o -path './.expo' \
         -o -path './target' -o -path './vendor' \
         -o -path './src/route-tree.gen.ts' \
         -o -path './worker-configuration.d.ts' \) -prune \
      -o -type f \
      \( -name '*.ts' -o -name '*.tsx' -o -name '*.mts' -o -name '*.cts' \
         -o -name '*.js' -o -name '*.jsx' -o -name '*.mjs' -o -name '*.cjs' \
         -o -name '*.astro' \) -print0 > "$list"
  else
    find . \
      \( -path './.git' -o -path './node_modules' -o -path './dist' \
         -o -path './build' -o -path './out' -o -path './coverage' \
         -o -path './.wrangler' -o -path './.next' -o -path './.expo' \
         -o -path './target' -o -path './vendor' \) -prune \
      -o -type f -name '*.rs' -print0 > "$list"
  fi
}

gr_ls_repo_lists() {
  local script_list rust_list errfile status errtext
  script_list=$1
  rust_list=$2
  errfile=$(mktemp)
  gr_ls_find_sources "$script_list" script 2>"$errfile"
  status=$?
  if [ "$status" -eq 0 ]; then
    gr_ls_find_sources "$rust_list" rust 2>>"$errfile"
    status=$?
  fi
  if [ "$status" -ne 0 ] || [ -s "$errfile" ]; then
    errtext=$(cat "$errfile")
    rm -f "$errfile"
    gr_fatal "lint-suppressions source inventory failed: ${errtext:-find exited $status}"
  fi
  rm -f "$errfile"
}

# gr_ls_scan_batch <nul-list> <kind> — one grep process per language family,
# then one violation per matching file. The sh wrapper absorbs grep's normal
# no-match status (1) but preserves real failures through xargs.
gr_ls_scan_batch() {
  local list kind pattern hitfile errfile status errtext path
  list=$1
  kind=$2
  [ -s "$list" ] || return 0
  hitfile=$(mktemp)
  errfile=$(mktemp)
  [ "$kind" = 'script' ] && pattern=$GR_LS_SCRIPT_CANDIDATE || pattern=$GR_LS_RUST_CANDIDATE
  xargs -0 sh -c 'p1=$1; shift; grep -E -I -l -Z -e "$p1" -- "$@" || [ $? -eq 1 ]' \
    sh "$pattern" < "$list" > "$hitfile" 2>"$errfile"
  status=$?
  errtext=$(cat "$errfile")
  rm -f "$errfile"
  if [ -n "$errtext" ] || [ "$status" -ne 0 ]; then
    rm -f "$hitfile"
    gr_fatal "lint-suppressions $kind repo scan failed: ${errtext:-batch exited $status}"
  fi
  while IFS= read -r -d '' path; do
    path=${path#./}
    gr_ls_scan "$path"
  done < "$hitfile"
  rm -f "$hitfile"
}

gr_check_lint_suppressions() {
  local mode path script_list rust_list
  mode=$1
  path=${2-}
  if [ "$mode" = 'file' ]; then
    gr_ls_scan "$path"
    return 0
  fi

  script_list=$(mktemp)
  rust_list=$(mktemp)
  gr_ls_repo_lists "$script_list" "$rust_list"
  gr_ls_scan_batch "$script_list" script
  gr_ls_scan_batch "$rust_list" rust
  rm -f "$script_list" "$rust_list"
}
