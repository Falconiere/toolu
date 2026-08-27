#!/usr/bin/env bash
# Shared detection helpers for project-agnostic hooks.
# Source via:   . "${BASH_SOURCE%/*}/../lib/detect.sh"

_TOOLU_DETECT_LIB_DIR="$(cd "${BASH_SOURCE%/*}" && pwd)"
# shellcheck source=host.sh
. "$_TOOLU_DETECT_LIB_DIR/host.sh"

# Print the absolute project root (git toplevel) or "" if not in a git repo.
detect_project_root() {
  git rev-parse --show-toplevel 2>/dev/null || true
}

# Print the project name (basename of the git toplevel) or "" if not in a git repo.
# Returns 0 even outside a git repo: a bare `[ -n "$root" ] && basename` would
# exit 1 when root is empty, which under `set -e` aborts callers before their
# own "unknown"-style fallback can run.
detect_project_name() {
  local root
  root=$(detect_project_root)
  if [ -n "$root" ]; then basename "$root"; fi
}

# Print the package manager: bun | pnpm | npm | yarn | "" (none detected).
detect_node_pm() {
  local root
  root=$(detect_project_root)
  [ -z "$root" ] && return 0
  [ -f "$root/bun.lock" ]          && echo bun && return
  [ -f "$root/bun.lockb" ]         && echo bun && return
  [ -f "$root/pnpm-lock.yaml" ]    && echo pnpm && return
  [ -f "$root/yarn.lock" ]         && echo yarn && return
  [ -f "$root/package-lock.json" ] && echo npm && return
}

# Echo "rust" if a Cargo.toml exists at the project root.
detect_rust() {
  local root
  root=$(detect_project_root)
  [ -z "$root" ] && return 0
  [ -f "$root/Cargo.toml" ] && echo rust
}

# Echo "ts" if a tsconfig*.json exists anywhere in the project.
detect_ts() {
  local root
  root=$(detect_project_root)
  [ -z "$root" ] && return 0
  git -C "$root" ls-files '**/tsconfig*.json' 'tsconfig*.json' 2>/dev/null \
    | grep -q . && echo ts
}

# Echo "comemory" if the comemory CLI is on PATH.
detect_comemory() {
  command -v comemory >/dev/null 2>&1 && echo comemory
}

# Minimum comemory version toolu targets. The wrapper relies on the full
# verb surface (search/save/list/summary, the retrieval-quality loop
# feedback/mine/tune/eval/prune/gc/rebuild, and comemory search-code/index-code/
# graph). 0.8.0 is the current release; bump this constant when comemory ships a
# newer one that toolu should rely on.
COMEMORY_MIN_VERSION="0.8.0"

# Echo the installed comemory version (e.g. "0.8.0"), or nothing.
# Returns 1 when the CLI is absent or the version can't be parsed.
comemory_version() {
  command -v comemory >/dev/null 2>&1 || return 1
  local v
  # Pin to comemory's OWN version token (`comemory <X.Y.Z>`), not any X.Y.Z in
  # the output, so a future `--version` that also prints a dep version (e.g.
  # "built against sqlite 3.45.0, version 0.8.0") can't match the wrong number.
  v=$(comemory --version 2>/dev/null \
        | grep -oE 'comemory[[:space:]]+v?[0-9]+\.[0-9]+\.[0-9]+' \
        | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
  [ -n "$v" ] && { echo "$v"; return 0; }
  return 1
}

# Compare the installed comemory against $COMEMORY_MIN_VERSION.
#   0 = installed >= minimum (good)
#   1 = installed <  minimum (outdated — caller should advise an upgrade)
#   2 = indeterminate (CLI absent or version unparseable — caller stays quiet)
comemory_version_ok() {
  local cur
  # comemory_version returns non-zero when the CLI is absent or unparseable.
  cur=$(comemory_version) || return 2
  # sort -V puts the lower version first; installed is OK iff the minimum is not
  # strictly greater (i.e. the minimum is the lower-or-equal of the two).
  [ "$(printf '%s\n%s\n' "$COMEMORY_MIN_VERSION" "$cur" | sort -V | head -1)" = "$COMEMORY_MIN_VERSION" ]
}

# Echo the project's TS linter: biome | oxc | eslint | "" (presence-only, by
# config-file at the git root; precedence biome > oxc > eslint). Used to point
# the agent at the real tool and to suppress our own overlapping nits — we
# never invoke the tool.
detect_ts_linter() {
  local root
  root=$(detect_project_root)
  [ -z "$root" ] && return 0
  { [ -f "$root/biome.json" ] || [ -f "$root/biome.jsonc" ]; }              && echo biome  && return
  [ -f "$root/.oxlintrc.json" ]                                             && echo oxc    && return
  { compgen -G "$root/.eslintrc*" >/dev/null 2>&1 \
      || compgen -G "$root/eslint.config.*" >/dev/null 2>&1; }              && echo eslint && return
}

# Echo "clippy" if a clippy config exists at the git root.
detect_clippy() {
  local root
  root=$(detect_project_root)
  [ -z "$root" ] && return 0
  { [ -f "$root/clippy.toml" ] || [ -f "$root/.clippy.toml" ]; } && echo clippy
}

# Echo the plugin spec ("name@marketplace") if installed at any scope.
# Reads ~/.claude/plugins/installed_plugins.json (Claude Code's authoritative
# install registry).
#
# Exit codes:
#   0 + spec on stdout — installed.
#   0 + empty stdout   — registry parsed; spec not present.
#   2 + empty stdout   — INDETERMINATE: registry missing, jq missing, or
#                        malformed JSON. Callers should suppress install
#                        warnings rather than spam users on a box where the
#                        registry was moved or jq was uninstalled.
#
# Usage:  detect_plugin_installed "code-simplifier@claude-plugins-official"
detect_plugin_installed() {
  local spec="$1"
  [ -z "$spec" ] && return 0
  if [ "$(toolu_host)" = codex ]; then
    toolu_codex_plugin_installed "$spec"
    return $?
  fi
  # Same config-root resolution as registry.sh, so the install gate and the
  # registry modules it gates follow CLAUDE_CONFIG_DIR together.
  local registry="${CLAUDE_PLUGINS_REGISTRY:-${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/plugins/installed_plugins.json}"
  [ -f "$registry" ] || return 2
  command -v jq >/dev/null 2>&1 || return 2
  # Guard against malformed registry (top-level `plugins` missing or wrong
  # type) — treat as indeterminate, not "not installed".
  if ! jq -e '.plugins | type == "object"' "$registry" >/dev/null 2>&1; then
    return 2
  fi
  # `has($s)` is tolerant of value type — array, object, number, or null at
  # the spec key all parse cleanly. The prior `.plugins[$s] | length > 0`
  # filter errored on non-array values and silently false-positived a WARN
  # if Claude Code ever wrote a non-array there.
  if jq -e --arg s "$spec" '.plugins | has($s)' "$registry" >/dev/null 2>&1; then
    echo "$spec"
    return 0
  fi
  return 0
}

# Echo "ast-grep" if either `sg` or `ast-grep` is on PATH.
detect_ast_grep() {
  if command -v sg >/dev/null 2>&1 || command -v ast-grep >/dev/null 2>&1; then
    echo ast-grep
  fi
}

# Return the base branch from origin/HEAD, or "main" if remote is missing.
# Takes an optional repo root as $1 — callers gating a push must pass the root
# the push targets, since a worktree and its main checkout can sit on different
# branches with different origin/HEAD resolution.
detect_base_branch() {
  local root ref
  root="${1:-$(detect_project_root)}"
  [ -z "$root" ] && { echo main; return; }
  ref=$(git -C "$root" symbolic-ref --quiet refs/remotes/origin/HEAD 2>/dev/null)
  if [ -n "$ref" ]; then
    echo "${ref#refs/remotes/origin/}"
  else
    echo main
  fi
}

# Print the data dir for settings/ lookups. Override with $TOOLU_SETTINGS_DIR.
toolu_settings_dir() {
  if [ -n "${TOOLU_SETTINGS_DIR:-}" ]; then
    echo "$TOOLU_SETTINGS_DIR"
  elif [ -d "${HOME}/.claude/settings" ]; then
    echo "${HOME}/.claude/settings"
  else
    local self
    self=$(cd "${BASH_SOURCE%/*}/.." && pwd)
    echo "$self/../settings"
  fi
}

# Strip heredoc bodies from a shell command read on stdin.
#
# Handles `<<TAG`, `<<-TAG` (tab-stripped form), quoted/unquoted tags, and
# tolerates trailing redirections/pipes on the heredoc-start line
# (e.g. `<<EOF > /tmp/x`, `<<EOF | tee`). Any [A-Za-z_][A-Za-z0-9_]* identifier
# is accepted as a tag — the previous sed recipe hardcoded `EOF` and silently
# missed `<<-EOF`, `<<END`, or `<<EOF > file`.
#
# Why this matters: bash-commands.sh, quality-gate.sh, push-review.sh, and
# search-nudge.sh all run their deny/match patterns over the command. Without
# stripping the heredoc body, prose like a commit message containing the
# substring `cargo test` would false-positive a deny rule.
strip_heredocs() {
  awk '
    BEGIN { in_heredoc = 0; tag = ""; tag_tab = "" }
    {
      if (in_heredoc) {
        line = $0
        if (line == tag || line == tag_tab) { in_heredoc = 0; tag = ""; tag_tab = "" }
        next
      }
      if (match($0, /<<-?[ \t]*"?'\''?[A-Za-z_][A-Za-z0-9_]*"?'\''?/)) {
        m = substr($0, RSTART, RLENGTH)
        # Strip leading `<<`, optional `-`, quotes, and surrounding whitespace.
        gsub(/^<<-?[ \t]*"?'\''?/, "", m)
        gsub(/"?'\''?$/, "", m)
        tag = m
        tag_tab = "\t" m
        in_heredoc = 1
        print
        next
      }
      print
    }
  '
}

# Echo a filesystem-safe slug for a branch name: '/'→'_', strip to
# [a-zA-Z0-9_-], empty → "_default". Used to key per-branch transient state
# files (push-review, plan-ledger). Takes the branch name as $1.
branch_slug() {
  local branch="$1"
  local slug
  slug=$(echo "$branch" | tr '/' '_' | tr -cd 'a-zA-Z0-9_-')
  [[ -z "$slug" ]] && slug="_default"
  echo "$slug"
}

# Split a command string into simple statements, honouring shell quoting.
#
# Statements keep their original text (quotes included) — the tokenizer below
# removes those. Only UNQUOTED `;`, `&&`, `||`, `|`, `&` and newlines separate
# statements, so an operator inside a quoted argument does not split it.
#
# Command-substitution bodies — `$(...)` and backticks — are lifted out and
# emitted as statements in their own right, because `foo $(git push)` really
# does push. Inside single quotes nothing is substituted, so those are left as
# literal text.
#
# Results land in the global array $_TOOLU_STMTS (bash 3.2 has no nameref).
_toolu_split_statements() {
  local s="$1"
  local i=0 n=${#s} c nxt q="" cur="" body depth start
  _TOOLU_STMTS=()

  _toolu_flush_stmt() {
    case "$cur" in
      *[![:space:]]*) _TOOLU_STMTS+=("$cur") ;;
    esac
    cur=""
  }

  while [ "$i" -lt "$n" ]; do
    c="${s:$i:1}"

    # Single quotes: literal to the closing quote. No operators, no expansion.
    if [ "$q" = "'" ]; then
      cur+="$c"
      [ "$c" = "'" ] && q=""
      i=$((i + 1))
      continue
    fi

    # Double quotes: operators are literal, but substitutions still run.
    if [ "$q" = '"' ]; then
      if [ "$c" = '\' ]; then
        cur+="${s:$i:2}"
        i=$((i + 2))
        continue
      fi
      if [ "$c" = '"' ]; then
        cur+="$c"
        q=""
        i=$((i + 1))
        continue
      fi
      if [ "$c" = '$' ] && [ "${s:$((i + 1)):1}" = "(" ]; then
        depth=1
        start=$((i + 2))
        i=$start
        while [ "$i" -lt "$n" ] && [ "$depth" -gt 0 ]; do
          case "${s:$i:1}" in
            "(") depth=$((depth + 1)) ;;
            ")") depth=$((depth - 1)) ;;
          esac
          i=$((i + 1))
        done
        body="${s:$start:$((i - start - 1))}"
        [ -n "$body" ] && _TOOLU_SUBSTS+=("$body")
        continue
      fi
      cur+="$c"
      i=$((i + 1))
      continue
    fi

    # Unquoted.
    case "$c" in
      "'"|'"')
        q="$c"
        cur+="$c"
        ;;
      '\')
        cur+="${s:$i:2}"
        i=$((i + 1))
        ;;
      '`')
        start=$((i + 1))
        i=$start
        while [ "$i" -lt "$n" ] && [ "${s:$i:1}" != '`' ]; do
          i=$((i + 1))
        done
        body="${s:$start:$((i - start))}"
        [ -n "$body" ] && _TOOLU_SUBSTS+=("$body")
        ;;
      '$')
        if [ "${s:$((i + 1)):1}" = "(" ]; then
          depth=1
          start=$((i + 2))
          i=$start
          while [ "$i" -lt "$n" ] && [ "$depth" -gt 0 ]; do
            case "${s:$i:1}" in
              "(") depth=$((depth + 1)) ;;
              ")") depth=$((depth - 1)) ;;
            esac
            i=$((i + 1))
          done
          body="${s:$start:$((i - start - 1))}"
          [ -n "$body" ] && _TOOLU_SUBSTS+=("$body")
          continue
        fi
        cur+="$c"
        ;;
      ';'|'&'|'|'|$'\n')
        nxt="${s:$((i + 1)):1}"
        if { [ "$c" = '&' ] && [ "$nxt" = '&' ]; } || { [ "$c" = '|' ] && [ "$nxt" = '|' ]; }; then
          i=$((i + 1))
        fi
        _toolu_flush_stmt
        ;;
      *)
        cur+="$c"
        ;;
    esac
    i=$((i + 1))
  done
  _toolu_flush_stmt
  unset -f _toolu_flush_stmt
}

# Tokenize ONE statement into argv, dropping quote characters and keeping a
# quoted run as a single token. This is what makes prose safe: in
# `echo "no git push rules"` the quoted text is one argument token, so the
# command is `echo`, not `git`.
#
# Results land in the global array $_TOOLU_TOKENS.
_toolu_statement_tokens() {
  local s="$1"
  local i=0 n=${#s} c q="" tok="" started=0
  _TOOLU_TOKENS=()
  while [ "$i" -lt "$n" ]; do
    c="${s:$i:1}"
    if [ -n "$q" ]; then
      if [ "$c" = "$q" ]; then
        q=""
      elif [ "$c" = '\' ] && [ "$q" = '"' ]; then
        tok+="${s:$((i + 1)):1}"
        i=$((i + 1))
      else
        tok+="$c"
      fi
      i=$((i + 1))
      continue
    fi
    case "$c" in
      "'"|'"')
        q="$c"
        started=1
        ;;
      '\')
        tok+="${s:$((i + 1)):1}"
        i=$((i + 1))
        started=1
        ;;
      ' '|$'\t'|$'\n')
        if [ "$started" = 1 ]; then
          _TOOLU_TOKENS+=("$tok")
          tok=""
          started=0
        fi
        ;;
      *)
        tok+="$c"
        started=1
        ;;
    esac
    i=$((i + 1))
  done
  [ "$started" = 1 ] && _TOOLU_TOKENS+=("$tok")
  return 0
}

# Print the git subcommand a statement invokes, or nothing.
#
# Git's global options sit between `git` and the subcommand, and two of them
# (-C, -c) take a separate value — consuming those is what keeps
# `git -c push.default=simple push` a push and `git commit -m "push"` a commit.
_toolu_git_subcommand() {
  _toolu_statement_tokens "$1"
  [ "${#_TOOLU_TOKENS[@]}" -gt 1 ] || return 1
  [ "${_TOOLU_TOKENS[0]}" = "git" ] || return 1

  local i=1 t
  while [ "$i" -lt "${#_TOOLU_TOKENS[@]}" ]; do
    t="${_TOOLU_TOKENS[$i]}"
    case "$t" in
      -C|-c)
        i=$((i + 2))
        ;;
      --*=*|--*|-?)
        i=$((i + 1))
        ;;
      *)
        printf '%s' "$t"
        return 0
        ;;
    esac
  done
  return 1
}

# Return 0 iff COMMAND ($1) invokes `git <SUBCOMMAND>` ($2) anywhere.
#
# Structural, not textual. The old regex matched the raw string, so any prose
# containing the two words in sequence — `echo "no git push rules"`, a commit
# message, a doc line — fired the push gates. Resolving the actual command of
# each statement is the difference between "these words appear" and "this runs".
toolu_runs_git_subcommand() {
  local cmd sub="$2" stmt found=1
  cmd=$(printf '%s\n' "$1" | strip_heredocs)

  _TOOLU_SUBSTS=()
  _toolu_split_statements "$cmd"
  local -a stmts=("${_TOOLU_STMTS[@]}")
  local -a substs=("${_TOOLU_SUBSTS[@]}")

  for stmt in ${stmts[@]+"${stmts[@]}"}; do
    if [ "$(_toolu_git_subcommand "$stmt")" = "$sub" ]; then
      found=0
      break
    fi
  done

  # A substitution body is its own command line; scan each recursively so a
  # push hidden in `$(...)` is not missed.
  if [ "$found" -ne 0 ]; then
    for stmt in ${substs[@]+"${substs[@]}"}; do
      if toolu_runs_git_subcommand "$stmt" "$sub"; then
        found=0
        break
      fi
    done
  fi
  return "$found"
}

# Return 0 iff COMMAND ($1) runs `git push`.
#
# Structural, via toolu_runs_git_subcommand: the command is parsed into
# statements and git's real subcommand is resolved, so the two words appearing
# in prose (`echo "no git push rules"`, a commit message, a doc line) is not a
# push. Heredoc bodies are stripped first, and `$(...)`/backtick bodies are
# scanned as commands of their own.
is_git_push() {
  toolu_runs_git_subcommand "$1" push
}

# Return 0 iff COMMAND ($1) runs `git commit`.
#
# Same structural detection as is_git_push — `git commit-tree` is a different
# subcommand and does not match, and `echo "remember to git commit"` is prose.
is_git_commit() {
  toolu_runs_git_subcommand "$1" commit
}

# Echo the absolute root of the repo a `git push` command ($1) targets.
#
# The hook's own cwd is not authoritative: `git -C <path> push` targets another
# checkout entirely. Resolving from the command keeps a worktree push gated
# against the worktree's own state, instead of reading the main checkout's state
# file (wrong branch, wrong diff) or — when the main checkout happens to sit on
# the base branch — skipping the gate altogether.
#
# Falls back to the cwd's toplevel, then the host-native project root, then cwd.
push_target_root() {
  local command="$1" dir root
  local -a cd_args=()

  # Split on statement separators first: in `git -C a status && git -C b push`
  # only the `push` segment's own -C counts. Within that segment, collect EVERY
  # -C preceding `push` and replay them all — git applies them cumulatively
  # (each relative to the last), so honouring only the first or last would
  # resolve a different directory than the push itself uses.
  while IFS= read -r dir; do
    [ -n "$dir" ] || continue
    dir=${dir%\"}
    dir=${dir#\"}
    cd_args+=(-C "$dir")
  done < <(printf '%s\n' "$command" | strip_heredocs \
    | awk '{ gsub(/&&|\|\||;|\|/, "\n"); print }' \
    | awk '{
        for (i = 1; i <= NF; i++)
          if ($i == "push") {
            for (j = 1; j < i; j++) if ($j == "-C") print $(j + 1)
            exit
          }
      }')

  if [ ${#cd_args[@]} -gt 0 ]; then
    root=$(git "${cd_args[@]}" rev-parse --show-toplevel 2>/dev/null)
  fi
  [ -n "$root" ] || root=$(git rev-parse --show-toplevel 2>/dev/null)
  [ -n "$root" ] || root=$(toolu_project_root)
  [ -n "$root" ] || root=$(pwd)
  echo "$root"
}

# Read non-comment non-blank lines from a settings file. Returns 0 with no
# output if the file is missing.
read_list() {
  [ -f "$1" ] || return 0
  grep -vE '^\s*(#|$)' "$1"
}

# count_code_lines FILE  ->  lines of real code (blank lines and comments
# excluded). Handles // line comments and /* ... */ blocks (incl. multi-line and
# inline), for both TS and Rust (/// and //! reduce to // and are dropped).
# Heuristic: does not track // or /* inside string literals — consistent with
# the other comment-stripping passes in the quality modules. Known edge: a
# string like `let s = "/* x";` flips block mode on and under-counts following
# code until a `*/` appears. Rare in practice; full literal-aware parsing isn't
# worth it here. Lives in detect.sh (not quality-config.sh) so the lang modules
# get one honest definition with no fallback — they already hard-require detect.sh.
count_code_lines() {
  awk '
    BEGIN { inblock=0; n=0 }
    {
      line=$0
      if (inblock) {
        idx=index(line,"*/")
        if (idx>0) { line=substr(line, idx+2); inblock=0 } else next
      }
      while ((s=index(line,"/*"))>0) {
        rest=substr(line, s+2); e=index(rest,"*/")
        if (e>0) { line=substr(line,1,s-1) substr(rest, e+2) }
        else { line=substr(line,1,s-1); inblock=1; break }
      }
      c=index(line,"//"); if (c>0) line=substr(line,1,c-1)
      gsub(/^[ \t]+|[ \t]+$/, "", line)
      if (length(line)>0) n++
    }
    # If we ended still inside a /* block, an unterminated comment OR (more
    # likely) a string literal containing /* flipped block mode on and swallowed
    # the rest of the file. Undercounting there would let an oversized file slip
    # the size gate, so fall back to the raw line count — fail toward flagging.
    # Intentional: the raw NR over-counts (it includes the blanks/comments we
    # normally exclude). That is the fail-toward-flagging choice — better to
    # over-count and flag than under-count and let an oversized file pass.
    END { if (inblock) print NR; else print n }
  ' "$1" 2>/dev/null
}

# Return 0 if the file has more `/*` openers than `*/` closers — i.e. an
# unterminated block comment, or (more often) a string literal containing `/*`
# that confuses count_code_lines into the raw-line-count fallback. Lets the lang
# modules tell the user the size figure is approximate, not exact.
has_unterminated_block() {
  [ -f "$1" ] || return 1
  local open close
  open=$(grep -o '/\*' "$1" 2>/dev/null | wc -l | tr -d ' ')
  close=$(grep -o '\*/' "$1" 2>/dev/null | wc -l | tr -d ' ')
  [ "${open:-0}" -gt "${close:-0}" ]
}

# Echo a repo-relative path for the given (potentially absolute) file_path.
# Falls back to the input unchanged if it cannot determine the project root
# or if the path is not under that root.
#
# Claude's Edit/Write tools send absolute paths; settings globs are written
# repo-relative. Without this helper, [[ /abs/path == hooks/lib/** ]] is
# always false, silently no-op'ing trusted-script protection.
to_relative_path() {
  local p="${1:-}"
  [ -z "$p" ] && { echo ""; return 0; }
  local root
  root=$(detect_project_root)
  if [ -n "$root" ]; then
    case "$p" in
      "$root"/*) echo "${p#"$root"/}" ;;
      *)         echo "$p" ;;
    esac
  else
    echo "$p"
  fi
}

# toolu_plugin_active SPEC
# Boolean wrapper over detect_plugin_installed, used to gate "registry" hook
# modules on whether the contributing plugin is still installed.
#
# SPEC is the full "name@marketplace" spec (same arg detect_plugin_installed
# takes — it matches by exact spec via `.plugins | has($spec)`).
#
# Returns:
#   0 = plugin installed, OR indeterminate (no manifest / jq missing /
#       malformed) -> fail open, so enforcement isn't silently lost on
#       environments without the install registry.
#   1 = plugin definitively absent (registry parsed, spec not present).
toolu_plugin_active() {
  local rc out
  out=$(detect_plugin_installed "$1")
  rc=$?
  # Indeterminate (exit 2): fail open.
  [ "$rc" -eq 2 ] && return 0
  # Installed: detect_plugin_installed echoes the spec.
  [ -n "$out" ] && return 0
  return 1
}
