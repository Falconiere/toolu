#!/usr/bin/env bash
# Deterministic config scaffolder for the rust-refactor command
# (spec docs/toolu/specs/2026-06-20-rust-quality-refactor-design.md, plan step 7).
#
# Merges the rust-quality templates into a target Cargo workspace WITHOUT
# clobbering: absent files are copied, present files are merged key/table-wise
# (existing keys, comments, and ordering are preserved verbatim — only missing
# top-level keys / [tables] are appended). It also propagates [workspace.lints]:
# the lint block is added to the root manifest and every member crate is opted in
# with `[lints] workspace = true`, so the lints are LIVE rather than inert.
#
#   rust-scaffold.sh --path <repo> [--templates <dir>]
#
#     --path <repo>        the target Cargo workspace root (required)
#     --templates <dir>    template source (default: plugins/rust-quality/templates)
#
# taplo is NOT a dependency: TOML edits are done by a section-aware bash/awk
# appender, never a reformat. cargo is used (when present) to resolve members.
#
# Idempotent: a second run is a no-op — every key/table/opt-in already present is
# detected and skipped. shellcheck-clean.

set -euo pipefail

# --------------------------------------------------------------------------
# Locate self + default template dir.
# --------------------------------------------------------------------------
SC_SELF="$(cd "${BASH_SOURCE[0]%/*}" && pwd)/${BASH_SOURCE[0]##*/}"
SC_SELF_DIR="${SC_SELF%/*}"
# plugins/rust-quality/scripts/ -> plugins/rust-quality/templates/
SC_DEFAULT_TEMPLATES="$(cd "$SC_SELF_DIR/../templates" && pwd)"

sc_die() { printf 'rust-scaffold: %s\n' "$1" >&2; exit "${2:-1}"; }
sc_info() { printf 'rust-scaffold: %s\n' "$1"; }

# --------------------------------------------------------------------------
# Argument parsing.
# --------------------------------------------------------------------------
OPT_PATH=""
OPT_TEMPLATES=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --path)        OPT_PATH="${2:-}"; shift 2 ;;
    --path=*)      OPT_PATH="${1#--path=}"; shift ;;
    --templates)   OPT_TEMPLATES="${2:-}"; shift 2 ;;
    --templates=*) OPT_TEMPLATES="${1#--templates=}"; shift ;;
    -h|--help)     sed -n '2,22p' "$SC_SELF"; exit 0 ;;
    *)             sc_die "unknown argument: $1" 2 ;;
  esac
done

[ -n "$OPT_PATH" ] || sc_die "--path <repo> is required" 2
[ -d "$OPT_PATH" ] || sc_die "not a directory: $OPT_PATH" 2
REPO="$(cd "$OPT_PATH" && pwd -P)"

TEMPLATES="${OPT_TEMPLATES:-$SC_DEFAULT_TEMPLATES}"
[ -d "$TEMPLATES" ] || sc_die "templates dir not found: $TEMPLATES" 2
TEMPLATES="$(cd "$TEMPLATES" && pwd -P)"

[ -f "$REPO/Cargo.toml" ] || sc_die "no Cargo.toml at workspace root: $REPO" 4

for t in rustfmt.toml clippy.toml deny.toml clippy-lints.toml \
         lefthook.rust.yml ci-rust.yml; do
  [ -f "$TEMPLATES/$t" ] || sc_die "missing template: $TEMPLATES/$t" 3
done

# --------------------------------------------------------------------------
# TOML helpers — section-aware, never reformatting.
#
# A "top-level key" is a `key = …` line that appears BEFORE the first `[table]`
# header (i.e. in the root table). A "[table]" is a `[name]` header line. We only
# ever APPEND what the target lacks; existing bytes are left untouched.
# --------------------------------------------------------------------------

# toml_top_keys FILE -> the bare names of root-table keys (one per line).
# Stops at the first table header. Ignores blanks/comments. Strips `key.sub`
# dotted keys to their first segment for presence purposes is NOT done — dotted
# root keys are compared whole (rare in these templates).
toml_top_keys() {
  awk '
    /^[[:space:]]*\[/ { exit }                 # first table header -> done
    /^[[:space:]]*#/  { next }                 # comment
    /^[[:space:]]*$/  { next }                 # blank
    {
      line = $0
      sub(/=.*/, "", line)                     # keep LHS of the first =
      gsub(/[[:space:]]/, "", line)
      if (line != "") print line
    }
  ' "$1"
}

# toml_tables FILE -> the header names of every [table] (without brackets).
# Portable awk (BSD + GNU): no third-arg match(); extracts the header via
# RSTART/RLENGTH and strips the brackets/whitespace by string ops.
toml_tables() {
  awk '
    /^[[:space:]]*\[[^]]+\][[:space:]]*$/ {
      name = $0
      sub(/^[[:space:]]*\[/, "", name)
      sub(/\][[:space:]]*$/, "", name)
      gsub(/[[:space:]]/, "", name)
      if (name != "" && name !~ /^\[/) print name   # skip [[array]] of tables
    }
  ' "$1"
}

# toml_has_top_key FILE KEY -> 0 if KEY is a root-table key in FILE.
toml_has_top_key() {
  toml_top_keys "$1" | grep -qxF "$2"
}

# toml_has_table FILE NAME -> 0 if [NAME] exists in FILE.
toml_has_table() {
  toml_tables "$1" | grep -qxF "$2"
}

# toml_table_block TEMPLATE NAME -> the full [NAME] block (header + body up to
# the next table header or EOF) extracted from TEMPLATE, verbatim. Portable awk.
toml_table_block() {
  awk -v want="$2" '
    function header_name(s,   name) {
      if (s !~ /^[[:space:]]*\[[^]]+\][[:space:]]*$/) return ""
      name = s
      sub(/^[[:space:]]*\[/, "", name)
      sub(/\][[:space:]]*$/, "", name)
      gsub(/[[:space:]]/, "", name)
      return name
    }
    {
      h = header_name($0)
      if (h != "") { intable = (h == want) }
      if (intable) print
    }
  ' "$1"
}

# toml_top_key_line TEMPLATE KEY -> the verbatim root-table line for KEY.
toml_top_key_line() {
  awk -v want="$2" '
    /^[[:space:]]*\[/ { exit }
    /^[[:space:]]*#/  { next }
    /^[[:space:]]*$/  { next }
    {
      line = $0
      sub(/=.*/, "", line)
      gsub(/[[:space:]]/, "", line)
      if (line == want) { print; exit }
    }
  ' "$1"
}

# file_ends_with_newline FILE -> 0 if the file's last byte is a newline (or the
# file is empty). Used so appends never glue onto a no-trailing-newline last line.
file_ends_with_newline() {
  [ -s "$1" ] || return 0
  [ -z "$(tail -c1 "$1")" ]
}

# append_block FILE  (block on stdin) — appends with a guaranteed blank-line gap.
append_block() {
  local file="$1"
  file_ends_with_newline "$file" || printf '\n' >> "$file"
  printf '\n' >> "$file"
  cat >> "$file"
}

# --------------------------------------------------------------------------
# Merge a standalone TOML template (rustfmt/clippy/deny) into the target.
#   absent  -> copy the template verbatim.
#   present -> append only the root-table keys and [tables] the target lacks,
#              preserving the user's existing values/comments/order.
# --------------------------------------------------------------------------
merge_standalone_toml() {
  local name="$1"
  local tmpl="$TEMPLATES/$name"
  local dst="$REPO/$name"

  if [ ! -f "$dst" ]; then
    cp "$tmpl" "$dst"
    sc_info "wrote $name"
    return 0
  fi

  local added=0 key table block line
  # Missing root-table keys (compared by bare name; appended verbatim).
  while IFS= read -r key; do
    [ -n "$key" ] || continue
    toml_has_top_key "$dst" "$key" && continue
    line="$(toml_top_key_line "$tmpl" "$key")"
    [ -n "$line" ] || continue
    if [ "$added" -eq 0 ]; then
      file_ends_with_newline "$dst" || printf '\n' >> "$dst"
      printf '\n# --- added by rust-scaffold (missing keys) ---\n' >> "$dst"
    fi
    printf '%s\n' "$line" >> "$dst"
    added=1
  done < <(toml_top_keys "$tmpl")

  # Missing [tables] (full block appended verbatim).
  while IFS= read -r table; do
    [ -n "$table" ] || continue
    toml_has_table "$dst" "$table" && continue
    block="$(toml_table_block "$tmpl" "$table")"
    [ -n "$block" ] || continue
    printf '%s' "$block" | append_block "$dst"
    added=1
  done < <(toml_tables "$tmpl")

  if [ "$added" -eq 1 ]; then
    sc_info "merged missing keys/tables into $name"
  else
    sc_info "$name already complete (no changes)"
  fi
}

# --------------------------------------------------------------------------
# Member resolution — from `cargo metadata` (authoritative) when cargo can parse
# the manifest, else from a `[workspace] members` glob walk, else the root crate
# itself (single-crate). Emits one absolute member-root dir per line.
# --------------------------------------------------------------------------
resolve_members() {
  local manifest="$REPO/Cargo.toml" meta roots
  if command -v cargo >/dev/null 2>&1 \
     && command -v jq >/dev/null 2>&1; then
    if meta="$(cargo metadata --format-version 1 --no-deps \
                 --manifest-path "$manifest" 2>/dev/null)" && [ -n "$meta" ]; then
      roots="$(jq -r '.packages[]? | select(.manifest_path != null)
                      | (.manifest_path | sub("/Cargo.toml$"; ""))' <<< "$meta")"
      if [ -n "$roots" ]; then printf '%s\n' "$roots"; return 0; fi
    fi
  fi
  # Fallback: parse `[workspace] members = [ "a", "crates/*" ]` and glob it.
  resolve_members_from_manifest
}

# Parse the `members = [...]` array out of the [workspace] table and expand each
# entry (supporting a trailing /* glob) to absolute dirs containing a Cargo.toml.
resolve_members_from_manifest() {
  local manifest="$REPO/Cargo.toml" entries any=0 e dir
  entries="$(awk '
    function header_name(s,   name) {
      if (s !~ /^[[:space:]]*\[[^]]+\][[:space:]]*$/) return ""
      name = s
      sub(/^[[:space:]]*\[/, "", name)
      sub(/\][[:space:]]*$/, "", name)
      gsub(/[[:space:]]/, "", name)
      return name
    }
    {
      h = header_name($0)
      if (h != "") { inws = (h == "workspace"); next }
      if (!inws) next
      if ($0 ~ /members[[:space:]]*=/) { collect = 1 }
      if (collect) {
        s = $0
        while (match(s, /"[^"]*"/)) {
          item = substr(s, RSTART + 1, RLENGTH - 2)
          if (item != "") print item
          s = substr(s, RSTART + RLENGTH)
        }
        if (s ~ /\]/) collect = 0
      }
    }
  ' "$manifest")"

  while IFS= read -r e; do
    [ -n "$e" ] || continue
    if [ "${e%/\*}" != "$e" ]; then
      # glob form: parent/* -> each immediate subdir with a Cargo.toml.
      local parent="$REPO/${e%/\*}"
      [ -d "$parent" ] || continue
      for dir in "$parent"/*/; do
        [ -f "${dir}Cargo.toml" ] || continue
        printf '%s\n' "${dir%/}"; any=1
      done
    else
      dir="$REPO/$e"
      [ -f "$dir/Cargo.toml" ] && { printf '%s\n' "$dir"; any=1; }
    fi
  done <<< "$entries"

  # No members table (single-crate / virtual w/o members) -> the root itself,
  # but only if it is an actual package (single-crate). A virtual manifest with
  # no members yields nothing (root gains [workspace.lints] only).
  if [ "$any" -eq 0 ] && grep -qE '^[[:space:]]*\[package\]' "$manifest"; then
    printf '%s\n' "$REPO"
  fi
}

# --------------------------------------------------------------------------
# [workspace.lints] propagation.
#   1. If the root Cargo.toml lacks [workspace.lints], append the clippy-lints
#      block (its [workspace.lints.clippy] + [workspace.lints.rust] tables).
#   2. Opt every member in: add `[lints]\nworkspace = true` to each member that
#      lacks a [lints] table (never duplicated). A single-crate root that gets
#      [workspace.lints] is also a member, so it is opted in by the same loop.
# --------------------------------------------------------------------------
propagate_workspace_lints() {
  local root_manifest="$REPO/Cargo.toml"

  if toml_has_table "$root_manifest" "workspace.lints" \
     || toml_has_table "$root_manifest" "workspace.lints.clippy" \
     || toml_has_table "$root_manifest" "workspace.lints.rust"; then
    sc_info "[workspace.lints] already present in root Cargo.toml"
  else
    append_block "$root_manifest" < "$TEMPLATES/clippy-lints.toml"
    sc_info "added [workspace.lints] to root Cargo.toml"
  fi

  local members member opted=0 total=0
  members="$(resolve_members)"
  while IFS= read -r member; do
    [ -n "$member" ] || continue
    [ -f "$member/Cargo.toml" ] || continue
    total=$((total + 1))
    if toml_has_table "$member/Cargo.toml" "lints"; then
      continue
    fi
    printf '[lints]\nworkspace = true\n' | append_block "$member/Cargo.toml"
    opted=$((opted + 1))
  done <<< "$members"
  sc_info "lints.workspace opt-in: $opted of $total member(s) updated"
}

# --------------------------------------------------------------------------
# Hooks: lefthook.yml — copy the template if absent, else merge the rust-check
# command into the existing pre-commit.commands without dropping other hooks.
# --------------------------------------------------------------------------
scaffold_lefthook() {
  local dst="$REPO/lefthook.yml"
  local tmpl="$TEMPLATES/lefthook.rust.yml"

  if [ ! -f "$dst" ]; then
    cp "$tmpl" "$dst"
    sc_info "wrote lefthook.yml"
    return 0
  fi

  if grep -qE '^[[:space:]]+rust-check:' "$dst"; then
    sc_info "lefthook.yml already has rust-check (no changes)"
    return 0
  fi

  # The rust-check command block from the template — every line under its
  # `commands:` key (the indented command entry), verbatim. Written to a temp
  # file so awk can read it via getline (BSD awk rejects a multi-line -v value).
  local block_file
  block_file="$(mktemp)"
  awk '
    /^[[:space:]]+commands:[[:space:]]*$/ { incmd = 1; next }
    incmd { print }
  ' "$tmpl" > "$block_file"

  if grep -qE '^pre-commit:' "$dst" && grep -qE '^[[:space:]]+commands:' "$dst"; then
    # Insert the rust-check block immediately after the FIRST `commands:` line
    # that belongs to the pre-commit table, preserving every other hook. A flag
    # tracks whether we are inside the top-level `pre-commit:` block (entered on
    # its header, left on the next column-0, non-comment line). The block is
    # streamed in line-by-line via getline for BSD/GNU awk portability.
    local tmp
    tmp="$(mktemp)"
    awk -v bf="$block_file" '
      /^pre-commit:/        { in_pc = 1; print; next }
      /^[^[:space:]#]/      { in_pc = 0 }       # a new top-level key
      {
        print
        if (!done && in_pc && $0 ~ /^[[:space:]]+commands:[[:space:]]*$/) {
          while ((getline bl < bf) > 0) print bl
          close(bf); done = 1
        }
      }
    ' "$dst" > "$tmp"
    # Unchecked mv would leave lefthook.yml truncated/half-written on a failed
    # atomic swap — abort loudly instead so the user's hooks are never corrupted.
    mv "$tmp" "$dst" || { rm -f "$tmp" "$block_file"; sc_die "failed to write $dst" 5; }
    rm -f "$block_file"
    sc_info "merged rust-check into existing lefthook.yml pre-commit"
  else
    rm -f "$block_file"
    # No pre-commit table (or no commands:) — append the whole template block
    # (its non-comment body, so the pre-commit table is created).
    # Build the non-comment, non-blank body into a temp first so a grep/write
    # failure can't leave lefthook.yml half-appended: only a fully-built block is
    # committed to $dst. (grep returns 1 on "no match", which is a real failure
    # here — the template body must be non-empty — so it is surfaced, not eaten.)
    local body
    body="$(mktemp)"
    if ! grep -vE '^[[:space:]]*#' "$tmpl" | grep -vE '^[[:space:]]*$' > "$body"; then
      rm -f "$body"
      sc_die "failed to read template body for lefthook.yml: $tmpl" 5
    fi
    file_ends_with_newline "$dst" || printf '\n' >> "$dst"
    printf '\n' >> "$dst"
    cat "$body" >> "$dst" || { rm -f "$body"; sc_die "failed to append to $dst" 5; }
    rm -f "$body"
    sc_info "appended rust pre-commit hook to lefthook.yml"
  fi
}

# --------------------------------------------------------------------------
# CI: .github/workflows/<name>.yml — write the template if absent.
# --------------------------------------------------------------------------
scaffold_ci() {
  local wf_dir="$REPO/.github/workflows"
  local dst="$wf_dir/rust.yml"
  local tmpl="$TEMPLATES/ci-rust.yml"

  if [ -f "$dst" ]; then
    sc_info ".github/workflows/rust.yml already present (no changes)"
    return 0
  fi
  mkdir -p "$wf_dir"
  cp "$tmpl" "$dst"
  sc_info "wrote .github/workflows/rust.yml"
}

# --------------------------------------------------------------------------
# Drive.
# --------------------------------------------------------------------------
sc_info "scaffolding $REPO (templates: $TEMPLATES)"
merge_standalone_toml rustfmt.toml
merge_standalone_toml clippy.toml
merge_standalone_toml deny.toml
propagate_workspace_lints
scaffold_lefthook
scaffold_ci
sc_info "done"
