#!/usr/bin/env bash
# detect-stack.sh — best-effort detector of a project's UI stack for the design plugin.
# Reads the ROOT manifest(s) (package.json / pubspec.yaml / app.json) and emits a JSON
# (default) or one-line summary describing platform, frameworks, styling, UI libs, mobile.
# Standalone: pure bash + POSIX grep/sed; uses `jq` only if present (force off with --no-jq
# or DESIGN_DETECT_NO_JQ=1). Undetectable input → platform "unknown" and exit 0.
set -euo pipefail

FORMAT="json"
NO_JQ="${DESIGN_DETECT_NO_JQ:-0}"
ROOT="."

while [ $# -gt 0 ]; do
  case "$1" in
    --json) FORMAT="json" ;;
    --summary) FORMAT="summary" ;;
    --no-jq) NO_JQ=1 ;;
    -h|--help) echo "usage: detect-stack.sh [--json|--summary] [--no-jq] [PATH]"; exit 0 ;;
    -*) echo "detect-stack: unknown flag '$1'" >&2; exit 2 ;;
    *) ROOT="$1" ;;
  esac
  shift
done

PKG="$ROOT/package.json"
PUBSPEC="$ROOT/pubspec.yaml"
APPJSON="$ROOT/app.json"

# use_jq — true only when jq is available AND not disabled. Guarded so it never aborts.
use_jq() {
  [ "$NO_JQ" = "1" ] && return 1
  command -v jq >/dev/null 2>&1 || return 1
  return 0
}

# pkg_has <name> — is <name> a (dev)dependency in package.json? Dual path, both guarded so a
# non-match returns 1 cleanly without tripping `set -e` (callers use it in `if`).
pkg_has() {
  local name="$1"
  [ -f "$PKG" ] || return 1
  if use_jq; then
    jq -e --arg d "$name" '((.dependencies // {})[$d] // (.devDependencies // {})[$d]) != null' "$PKG" >/dev/null 2>&1
    return $?
  fi
  # POSIX ERE: the dep appears as a quoted key followed by a colon.
  grep -Eq "\"$name\"[[:space:]]*:" "$PKG"
}

# file_has <file> <ere> — guarded grep against a file that may not exist.
file_has() {
  [ -f "$1" ] || return 1
  grep -Eq "$2" "$1"
}

frameworks=""; styling=""; ui_libs=""
rn=false; expo=false; flutter=false
ev_keys=""; ev_vals=""

add() { # add <list-var-name> <value> — append value to the named space-list if absent
  local cur="${!1}"                                  # indirect read (no eval; bash 3.2-safe)
  case " $cur " in *" $2 "*) ;; *) printf -v "$1" '%s' "${cur:+$cur }$2" ;; esac  # indirect write
}
evidence() { ev_keys="${ev_keys:+$ev_keys }$1"; ev_vals="${ev_vals:+$ev_vals|}$2"; }

# --- web frameworks (package.json) ---
if pkg_has react; then add frameworks react; evidence react "package.json:dependencies.react"; fi
if pkg_has next; then add frameworks next; evidence next "package.json:dependencies.next"; fi
if pkg_has vue; then add frameworks vue; evidence vue "package.json:dependencies.vue"; fi
if pkg_has svelte; then add frameworks svelte; evidence svelte "package.json:dependencies.svelte"; fi
if pkg_has "@angular/core"; then add frameworks angular; evidence angular "package.json:dependencies.@angular/core"; fi

# --- styling (package.json) ---
if pkg_has tailwindcss; then add styling tailwind; evidence tailwind "package.json:tailwindcss"; fi
if pkg_has styled-components; then add styling styled-components; fi
if pkg_has "@emotion/react" || pkg_has "@emotion/styled"; then add styling emotion; fi
if pkg_has sass || pkg_has node-sass; then add styling sass; fi
if pkg_has "@vanilla-extract/css"; then add styling vanilla-extract; fi

# --- ui libraries (best-effort) ---
if pkg_has "@mui/material"; then add ui_libs mui; fi
if pkg_has "@chakra-ui/react"; then add ui_libs chakra; fi
if pkg_has antd; then add ui_libs antd; fi
if pkg_has "react-native-paper"; then add ui_libs react-native-paper; fi

# --- mobile ---
if pkg_has react-native; then rn=true; add frameworks react-native; evidence react-native "package.json:react-native"; fi
if pkg_has expo; then expo=true; add frameworks expo; evidence expo "package.json:expo"; fi
if [ "$expo" = false ] && file_has "$APPJSON" "\"expo\""; then expo=true; add frameworks expo; evidence expo "app.json:expo"; fi
if file_has "$PUBSPEC" "^[[:space:]]*flutter:" || file_has "$PUBSPEC" "sdk:[[:space:]]*flutter"; then
  flutter=true; add frameworks flutter; evidence flutter "pubspec.yaml"
fi

# --- platform derivation ---
web=false; mobile=false
case " $frameworks " in *" react "*|*" next "*|*" vue "*|*" svelte "*|*" angular "*) web=true ;; esac
{ [ "$rn" = true ] || [ "$expo" = true ] || [ "$flutter" = true ]; } && mobile=true
# react-native pulls in "react", so a pure RN app must not be misread as web:
if [ "$mobile" = true ] && [ "$web" = true ]; then
  # web only counts if a browser framework beyond bare react is present
  case " $frameworks " in *" next "*|*" vue "*|*" svelte "*|*" angular "*) ;; *) web=false ;; esac
fi
if [ "$web" = true ] && [ "$mobile" = true ]; then platform="both"
elif [ "$web" = true ]; then platform="web"
elif [ "$mobile" = true ]; then platform="mobile"
else platform="unknown"; fi

# --- emit ---
# json_arr <space-list> — render a space-separated list as a JSON string array.
json_arr() {
  local out="" w
  for w in $1; do out="${out:+$out, }\"$w\""; done
  printf '[%s]' "$out"
}
json_evidence() {
  local out="" i=1 k v
  # Split values on | without touching IFS globally: `read -ra` scopes the split
  # to the one call, so a later expansion can't inherit a half-restored IFS.
  local vals=()
  IFS='|' read -ra vals <<< "$ev_vals"
  for k in $ev_keys; do                            # keys split on whitespace
    v="${vals[$((i-1))]:-}"; out="${out:+$out, }\"$k\": \"$v\""; i=$((i+1))
  done
  printf '{%s}' "$out"
}

if [ "$FORMAT" = "summary" ]; then
  printf 'platform=%s frameworks=%s styling=%s\n' \
    "$platform" "$(echo "$frameworks" | tr ' ' ',')" "$(echo "$styling" | tr ' ' ',')"
  exit 0
fi

printf '{\n'
printf '  "platform": "%s",\n' "$platform"
printf '  "frameworks": %s,\n' "$(json_arr "$frameworks")"
printf '  "styling": %s,\n' "$(json_arr "$styling")"
printf '  "uiLibraries": %s,\n' "$(json_arr "$ui_libs")"
printf '  "mobile": { "reactNative": %s, "expo": %s, "flutter": %s },\n' "$rn" "$expo" "$flutter"
printf '  "evidence": %s\n' "$(json_evidence)"
printf '}\n'
