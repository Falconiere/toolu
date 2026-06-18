#!/usr/bin/env bash
# Shared detection helpers for project-agnostic hooks.
# Source via:   . "${BASH_SOURCE%/*}/../lib/detect.sh"

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
  # Same config-root resolution as registry.sh, so the install gate and the
  # registry modules it gates follow CLAUDE_CONFIG_DIR together.
  local registry="${CLAUDE_PLUGINS_REGISTRY:-${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-${PI_CODING_AGENT_DIR:-$HOME/.claude}}}/plugins/installed_plugins.json}"
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
detect_base_branch() {
  local root ref
  root=$(detect_project_root)
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

# Return 0 iff the raw command string $1 is a `git push` (heredoc bodies
# stripped first so a `git push` inside a heredoc/commit-message is ignored).
# Boundary-anchored on both sides so `gitpush`/`git pushup` do not match.
is_git_push() {
  printf '%s\n' "$1" | strip_heredocs \
    | grep -qE '(^|\s|&&|\|\||;)git\s+push(\s|;|&|\||$)'
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
