#!/usr/bin/env bash
# debug-stack.sh — language-agnostic stack/backtrace summarizer for the debug skill's Observe step.
# Reads a stack trace / panic backtrace (stdin or --file) and surfaces APP frames first, collapsing
# framework/runtime/stdlib frames into a count. Never a per-language grammar — it keys on line SHAPE
# (a symbol and/or a file:line[:col]) and flags noise by path/symbol markers (/rustc/, node:internal,
# core::, std::, __rustc, <anonymous>, …). JS (symbol+loc on one line) and Rust (symbol then `at <path>`
# on the next line) both work. Unrecognized input → capped raw passthrough.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: debug-stack.sh [--file <path>] [--json]
  Reads a stack trace / backtrace from stdin or --file and prints APP frames first, runtime noise collapsed.
  --json    Emit {"app_frames":[...],"noise_frames":N,"recognized":bool} instead of text.
Env caps (override): DEBUG_MAX_FRAMES=20 DEBUG_MAX_LINES=100
EOF
}

file=""
json=0
while [ $# -gt 0 ]; do
  case "$1" in
    --file) file="${2:-}"; shift 2 || { echo "debug-stack.sh: --file needs a path" >&2; exit 2; } ;;
    --json) json=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "debug-stack.sh: unknown arg: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [ -n "$file" ]; then
  [ -r "$file" ] || { echo "debug-stack.sh: cannot read file: $file" >&2; exit 2; }
  exec < "$file"
elif [ -t 0 ]; then
  echo "debug-stack.sh: no input (pipe a trace or pass --file)" >&2
  exit 2
fi

awk -v json="$json" \
    -v maxfr="${DEBUG_MAX_FRAMES:-20}" \
    -v maxraw="${DEBUG_MAX_LINES:-100}" '
function trim(s){ sub(/^[ \t]+/,"",s); sub(/[ \t]+$/,"",s); return s }
function jesc(s){ gsub(/\\/,"\\\\",s); gsub(/"/,"\\\"",s); gsub(/\t/," ",s); return s }
# A frame is NOISE when its symbol or location points into a runtime/stdlib (language-agnostic markers).
function isnoise(text){ return (text ~ /\/rustc\/|node:internal|node:|core::|std::|__rustc|rust_begin_unwind|panic_fmt|panic_bounds_check|FnOnce::call_once|<anonymous>/) }
function emit(sym,loc,   txt){
  # sym/loc are the parts; txt is what we classify on. "func @ file:line" is the rendered frame.
  txt=sym " " loc
  if (isnoise(txt)) { noise++; return }
  if (sym!="" && loc!="") f=sym " @ " loc
  else if (loc!="")       f=loc
  else                    f=sym
  napp++; app[napp]=f
}
{
  raw[NR]=$0
  line=$0

  # --- Rust continuation FIRST: `             at <location>` pairs with the pending `N: symbol` ---
  if (pend && match(line, /^[ \t]*at[ \t]+/)) {
    emit(pend_sym, trim(substr(line, RSTART+RLENGTH))); pend=0; pend_sym=""; next
  }
  # A pending Rust symbol with no following `at` line: still a frame, no location.
  if (pend) { emit(pend_sym, ""); pend=0; pend_sym="" }

  # --- JS shape: a stack frame line begins with `at <symbol> (<location>)` or `at <location>` ---
  if (match(line, /^[ \t]*at[ \t]+/)) {
    rest=substr(line, RSTART+RLENGTH)
    sym=""; loc=""
    if (match(rest, /\(.*\)[ \t]*$/)) {           # at func (loc)
      sym=trim(substr(rest, 1, RSTART-1))
      loc=substr(rest, RSTART+1, RLENGTH-2)       # strip the surrounding ()
      sub(/[ \t]+$/,"",loc)
    } else {                                       # at loc   (no symbol)
      loc=trim(rest)
    }
    if (sym!="" || loc!="") { emit(sym, loc); next }
  }

  # --- Rust shape: `   N: <symbol>` then the NEXT line `             at <location>` ---
  if (match(line, /^[ \t]*[0-9]+:[ \t]+/)) {
    pend_sym=trim(substr(line, RSTART+RLENGTH)); pend=1; next
  }
}
END {
  if (pend) emit(pend_sym, "")        # flush a trailing pending Rust frame
  recognized = (napp + noise) > 0
  if (json) {
    printf "{"
    printf "\"app_frames\":["; for(i=1;i<=napp;i++){printf "%s\"%s\"", (i>1?",":""), jesc(app[i])} printf "],"
    printf "\"noise_frames\":%d,", noise
    printf "\"recognized\":%s", (recognized?"true":"false")
    printf "}\n"
    exit 0
  }
  if (!recognized) {
    print "debug-stack: no recognizable stack frames — raw input (capped):"
    cap = (NR<maxraw?NR:maxraw)
    for(i=1;i<=cap;i++) print raw[i]
    if (NR>maxraw) printf "... (+%d more lines)\n", NR-maxraw
    exit 0
  }
  shown = (napp<maxfr?napp:maxfr)
  print "APP FRAMES (" napp (napp>maxfr?"+":"") "):"
  for(i=1;i<=shown;i++) print "  - " app[i]
  if (napp>maxfr) printf "  ... (+%d more)\n", napp-maxfr
  if (noise>0) printf "(+%d framework/runtime frames collapsed)\n", noise
}
'
