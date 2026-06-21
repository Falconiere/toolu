#!/usr/bin/env bash
# Layering rules for the canonical Rust rule library — split out of
# rust-rules.sh to keep each file under ~300 lines. Sourced by rust-scan.sh and
# concatenated into the gate module by register.sh (same two-consumer contract
# as rust-rules.sh). Depends on rr_emit + rr_config_bool from rust-rules.sh.
#
# This file must stay shellcheck-clean.

# rule_layering_file FILE — per-file layering heuristic, sev=block autofix=manual.
# Applies only to a CHILD module file: a */src/* .rs that is NOT mod.rs / lib.rs
# / main.rs. Conservative on purpose (heuristics, not visibility resolution) to
# avoid false positives. Gate config enforceLayering (default true) toggles it.
#
# Two heuristics:
#   1. A top-level item declared bare `pub` (recommend `pub(super)` for a child
#      module). `pub(...)` restricted forms are already explicit and left alone.
#   2. A `use crate::a::b::c::…` path that reaches more than one module segment
#      deep — i.e. past a sibling's top-level module into its internals.
rule_layering_file() {
  local file="$1" base
  [ -f "$file" ] || return 0
  [ "$(rr_config_bool enforceLayering 1)" = "1" ] || return 0
  [[ "$file" == */src/* ]] || return 0
  base=$(basename "$file")
  case "$base" in
    mod.rs|lib.rs|main.rs) return 0 ;;
  esac

  # Heuristic 1 — bare `pub` on a top-level item (column 0, no leading indent so
  # nested/associated items inside an impl or fn are excluded). `pub(` restricted
  # forms and `pub use` re-exports are skipped.
  local bare
  bare=$(awk '
    /^pub[ \t]+(fn|struct|enum|trait|const|static|type|mod)[ \t]/ { print NR; exit }
  ' "$file" 2>/dev/null)
  if [ -n "$bare" ]; then
    rr_emit layering block "$file" "$bare" manual \
      "Child module exposes a bare 'pub' item — prefer 'pub(super)' (or 'pub(crate)') unless it is part of the crate's public API"
  fi

  # Heuristic 2 — `use crate::seg1::seg2::seg3…` reaching more than one segment
  # past the first into a sibling (4+ `::`-separated path components after
  # `crate::`). Conservative: only flags clearly deep reaches.
  local reach
  reach=$(awk '
    /^[ \t]*use[ \t]+crate::/ {
      p=$0
      sub(/^[ \t]*use[ \t]+/, "", p)
      sub(/[ \t]*;.*$/, "", p)
      sub(/[ \t]*\{.*$/, "", p)        # use crate::a::{b, c} — count up to the brace
      n=split(p, parts, "::")
      # parts[1]=crate. A reach of crate::a::b::c is n>=4 (crate,a,b,c).
      if (n >= 4) { print NR; exit }
    }
  ' "$file" 2>/dev/null)
  if [ -n "$reach" ]; then
    rr_emit layering block "$file" "$reach" manual \
      "Deep 'use crate::…' reach into a sibling's internals — import via the sibling module's public surface, not its private path"
  fi
}

# rule_layering_crate CRATE_ROOT — cross-file, repo-wide layering heuristic,
# sev=ADVISORY autofix=manual. This is the AC-17 best-effort import-graph check.
# It complements the per-file rule_layering_file: it looks ACROSS files at how a
# sibling module reaches into another sibling's internals.
#
# Approach (best-effort ast-grep import graph, NOT visibility resolution):
#   For each module directory `<mod>/` under the crate's src/ that has a
#   `<mod>/mod.rs`, the re-export SURFACE is the set of leaf names a `pub use`
#   in that mod.rs exposes (`pub use self::api::PublicThing;` -> PublicThing).
#   A sibling source file's NON-pub `use crate::<mod>::<item>::…` (or
#   `use super::<mod>::<item>::…`) is flagged when <item> — the segment right
#   after the module name — is NOT in <mod>'s re-export surface, i.e. it reaches
#   past the module's public face into its private internals.
#
# Conservative by design — when ANYTHING is uncertain we DO NOT flag (a missed
# violation is far better than a false positive on clean re-export usage):
#   * A module whose mod.rs has NO `pub use` has an unknown surface -> skipped.
#   * KNOWN LIMITS, all of which TAINT a module so reaches into it are NEVER
#     flagged (conservative miss, never a false positive):
#       - GLOB re-exports  `pub use self::x::*;`        (leaf set unknown)
#       - RENAMED re-exports `pub use self::x::y as z;` (visible name rewritten)
#       - MACRO-GENERATED paths are not parsed as `use` items at all, so an item
#         a macro brings into the surface is invisible here -> we cannot prove a
#         reach is illegal, so such modules are likewise left unflagged.
#   These are documented misses; the rule trades completeness for zero false
#   positives so AC-17 can stay non-blocking (advisory) and deterministically green.
#
# Honors the enforceLayering config toggle (default true), like rule_layering_file.
# Degrades silently (emits nothing) when ast-grep or jq is unavailable.
rule_layering_crate() {
  local crate_root="$1" src
  [ -n "$crate_root" ] || return 0
  [ -d "$crate_root" ] || return 0
  [ "$(rr_config_bool enforceLayering 1)" = "1" ] || return 0
  command -v ast-grep >/dev/null 2>&1 || return 0
  command -v jq >/dev/null 2>&1 || return 0
  src="$crate_root/src"
  [ -d "$src" ] || return 0

  # ast-grep meta-variable patterns. The `$P` capture sigil is assembled from a
  # variable so the literal pattern carries no `$` inside single quotes (keeps
  # the file shellcheck-clean without a disable directive); `$P` is ast-grep
  # syntax, NOT a shell expansion.
  local mv='$'
  local pat_pubuse="pub use ${mv}P;"
  local pat_use="use ${mv}P;"

  # --- Phase 1: build each sibling module's re-export surface from its mod.rs.
  # surfaces holds one record per module dir:  <mod><TAB>tainted<TAB>name,name,…
  # tainted=1 marks a module we must never flag reaches into (glob/rename, or no
  # surface). The module key is the directory name (the path segment a sibling's
  # `use crate::<mod>::…` names).
  local surfaces="" moddir mod_name mod_rs surface tainted paths p
  while IFS= read -r mod_rs; do
    [ -n "$mod_rs" ] || continue
    moddir=$(dirname "$mod_rs")
    mod_name=$(basename "$moddir")
    # Only direct children of src/ are siblings addressed as `crate::<mod>`.
    [ "$(dirname "$moddir")" = "$src" ] || continue

    # Collect this mod.rs's `pub use` path texts via ast-grep (code-shape match).
    paths=$(ast-grep run --lang rust --pattern "$pat_pubuse" --json=compact \
              "$mod_rs" 2>/dev/null \
            | jq -r '.[]?.metaVariables.single.P.text // empty' 2>/dev/null)

    surface=""
    tainted=0
    if [ -z "$paths" ]; then
      # No re-export surface declared -> unknown contract -> never flag.
      tainted=1
    else
      while IFS= read -r p; do
        [ -n "$p" ] || continue
        # KNOWN LIMITS: a glob (`::*`) or a rename (` as `) makes the visible
        # leaf set unknowable -> taint the whole module (conservative miss).
        case "$p" in
          *"::*"|*"*") tainted=1; continue ;;
          *" as "*)    tainted=1; continue ;;
        esac
        # Extract leaf name(s). Two shapes:
        #   prefix::Leaf            -> Leaf
        #   prefix::{A, B, C}       -> A B C
        if [[ "$p" == *"{"* ]]; then
          local group items it
          group=${p##*\{}; group=${group%%\}*}
          items=${group//,/ }
          for it in $items; do
            it=${it// /}
            [ -n "$it" ] && surface+="$it"$'\n'
          done
        else
          surface+="${p##*::}"$'\n'
        fi
      done <<< "$paths"
    fi
    # Squeeze the surface to a sorted, comma-joined, unique CSV for storage.
    local csv=""
    if [ -n "$surface" ]; then
      csv=$(printf '%s\n' "$surface" | awk 'NF' | sort -u | paste -sd, -)
    fi
    surfaces+="$mod_name"$'\t'"$tainted"$'\t'"$csv"$'\n'
  done <<< "$(find "$src" -mindepth 2 -name mod.rs -type f 2>/dev/null | sort)"

  [ -n "$surfaces" ] || return 0

  # --- Phase 2: scan every src/ .rs file's NON-pub `use` for a reach into a
  # sibling module's non-reexported internals. `use $P;` (ast-grep) deliberately
  # excludes `pub use`, so a file's own re-exports never count as a reach.
  local rs_file use_json line path seg mod_seg item rec mtaint msurface
  while IFS= read -r rs_file; do
    [ -n "$rs_file" ] || continue
    # ast-grep emits one object per non-pub `use …;`; pull (line, path-text).
    use_json=$(ast-grep run --lang rust --pattern "$pat_use" --json=compact \
                 "$rs_file" 2>/dev/null \
               | jq -r '.[]? | "\((.range.start.line + 1))\t\(.metaVariables.single.P.text)"' \
                 2>/dev/null)
    [ -n "$use_json" ] || continue

    while IFS=$'\t' read -r line path; do
      [ -n "$line" ] && [ -n "$path" ] || continue
      # Normalize the leading anchor to the sibling module + the reached segment.
      #   crate::<mod>::<item>::…   -> mod=<mod> item=<item>
      #   super::<mod>::<item>::…   -> mod=<mod> item=<item>
      case "$path" in
        crate::*) seg=${path#crate::} ;;
        super::*) seg=${path#super::} ;;
        *) continue ;;
      esac
      # Need at least <mod>::<item> — a bare `crate::<mod>` (re-import of the
      # module itself) reaches nothing private, so skip it.
      case "$seg" in
        *::*) ;;
        *) continue ;;
      esac
      mod_seg=${seg%%::*}
      item=${seg#"$mod_seg"::}
      item=${item%%::*}              # first segment after the module name
      # A brace right after the module (`crate::a::{x, y}`) is a multi-import; be
      # conservative and skip (resolving each leaf vs the surface is ambiguous
      # once nested groups appear).
      case "$item" in
        ""|*"{"*|"*"|\**) continue ;;
      esac

      # Intra-module reach is legal: a file INSIDE <mod>/ may touch its own
      # internals. Skip when the offending file lives under the target module.
      case "$rs_file" in
        "$src/$mod_seg/"*) continue ;;
      esac

      # Look the module up in the surfaces table.
      rec=$(printf '%s' "$surfaces" | awk -F '\t' -v m="$mod_seg" '$1 == m { print; exit }')
      [ -n "$rec" ] || continue                  # not a sibling module dir -> skip
      mtaint=$(printf '%s' "$rec" | cut -f2)
      msurface=$(printf '%s' "$rec" | cut -f3)
      [ "$mtaint" = "1" ] && continue            # glob/rename/no-surface -> never flag
      # The reached item is illegal iff it is absent from the re-export surface.
      case ",$msurface," in
        *",$item,"*) ;;                          # re-exported -> clean
        *)
          rr_emit layering advisory "$rs_file" "$line" manual \
            "Cross-module reach: '$item' is not in module '$mod_seg''s re-export surface (mod.rs pub use) — import via '$mod_seg''s public face, not its private internals"
          ;;
      esac
    done <<< "$use_json"
  done <<< "$(find "$src" -name '*.rs' -type f 2>/dev/null | sort)"
}
