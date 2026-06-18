#!/usr/bin/env bash
# debug-testfail.sh — language-agnostic test-failure summarizer for the debug skill's Observe step.
# Reads a failing test transcript (stdin or --file), emits a compact, capped summary:
# failed test names, error/assertion lines, and code file:line locations. Never a per-language grammar —
# it keys on line shape only (works on bun/jest, cargo, and similar). Unrecognized input → capped raw passthrough.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: debug-testfail.sh [--file <path>] [--json]
  Reads a test-failure transcript from stdin or --file and prints a compact summary.
  --json    Emit a JSON object {failures,errors,locations} instead of text.
Env caps (override): DEBUG_MAX_FAILURES=25 DEBUG_MAX_ERRORS=25 DEBUG_MAX_LOCATIONS=25 DEBUG_MAX_LINES=100
EOF
}

file=""
json=0
while [ $# -gt 0 ]; do
  case "$1" in
    --file) file="${2:-}"; shift 2 || { echo "debug-testfail.sh: --file needs a path" >&2; exit 2; } ;;
    --json) json=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "debug-testfail.sh: unknown arg: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [ -n "$file" ]; then
  [ -r "$file" ] || { echo "debug-testfail.sh: cannot read file: $file" >&2; exit 2; }
  exec < "$file"
elif [ -t 0 ]; then
  echo "debug-testfail.sh: no input (pipe a transcript or pass --file)" >&2
  exit 2
fi

awk -v json="$json" \
    -v maxf="${DEBUG_MAX_FAILURES:-25}" \
    -v maxe="${DEBUG_MAX_ERRORS:-25}" \
    -v maxl="${DEBUG_MAX_LOCATIONS:-25}" \
    -v maxraw="${DEBUG_MAX_LINES:-100}" '
function trim(s){ sub(/^[ \t]+/,"",s); sub(/[ \t]+$/,"",s); return s }
function jesc(s){ gsub(/\\/,"\\\\",s); gsub(/"/,"\\\"",s); gsub(/\t/," ",s); return s }
function add(arr,seen,n,max,val,   key){ val=trim(val); if(val=="")return n; key=val; if(key in seen)return n; if(n>=max){over[max==maxf?"f":(max==maxe?"e":"l")]++;return n} seen[key]=1; arr[++n]=val; return n }
{
  raw[NR]=$0
  line=$0

  # --- failed test NAMES (line-shape, not language) ---
  if (match(line, /\(fail\)[ \t]+/)) {                       # bun/jest:  (fail) NAME [1ms]
    name=substr(line, RSTART+RLENGTH); sub(/[ \t]*\[[^]]*\][ \t]*$/,"",name)
    nf=add(fails,fseen,nf,maxf,name)
  } else if (match(line, /(^|[ \t])test[ \t]+[^ \t]+[ \t]+\.\.\.[ \t]+FAILED/)) {  # cargo: test NAME ... FAILED
    n=line; sub(/^[ \t]*test[ \t]+/,"",n); sub(/[ \t]+\.\.\..*/,"",n)
    nf=add(fails,fseen,nf,maxf,n)
  } else if (match(line, /^[ \t]*(FAIL|✕|✗|×)[ \t]+/)) {     # generic fail markers
    name=substr(line, RSTART+RLENGTH)
    nf=add(fails,fseen,nf,maxf,name)
  }

  # --- ERROR / assertion lines ---
  if (line ~ /([Ee]rror:|panicked at|[Aa]ssertion|AssertionError|Expected:|Received:|^[ \t]*left:|^[ \t]*right:|thread .* panicked)/) {
    ne=add(errs,eseen,ne,maxe,line)
  }

  # --- code file:line[:col] locations (path with extension + :digits) ---
  s=line
  while (match(s, /[A-Za-z0-9_.\/\\-]+\.[A-Za-z]+:[0-9]+(:[0-9]+)?/)) {
    loc=substr(s, RSTART, RLENGTH)
    nl=add(locs,lseen,nl,maxl,loc)
    s=substr(s, RSTART+RLENGTH)
  }
}
END {
  recognized = (nf+ne+nl) > 0
  if (json) {
    printf "{"
    printf "\"failures\":["; for(i=1;i<=nf;i++){printf "%s\"%s\"", (i>1?",":""), jesc(fails[i])} printf "],"
    printf "\"errors\":[";   for(i=1;i<=ne;i++){printf "%s\"%s\"", (i>1?",":""), jesc(errs[i])}  printf "],"
    printf "\"locations\":[";for(i=1;i<=nl;i++){printf "%s\"%s\"", (i>1?",":""), jesc(locs[i])}  printf "],"
    printf "\"recognized\":%s", (recognized?"true":"false")
    printf "}\n"
    exit 0
  }
  if (!recognized) {
    print "debug-testfail: no recognizable test failures — raw input (capped):"
    cap = (NR<maxraw?NR:maxraw)
    for(i=1;i<=cap;i++) print raw[i]
    if (NR>maxraw) printf "... (+%d more lines)\n", NR-maxraw
    exit 0
  }
  print "FAILED TESTS (" nf (over["f"]?"+":"") "):"
  for(i=1;i<=nf;i++) print "  - " fails[i]
  if (over["f"]) print "  ... (+" over["f"] " more)"
  if (ne>0) { print "\nERRORS (" ne (over["e"]?"+":"") "):"; for(i=1;i<=ne;i++) print "  " errs[i]; if(over["e"]) print "  ... (+" over["e"] " more)" }
  if (nl>0) { print "\nLOCATIONS (" nl (over["l"]?"+":"") "):"; for(i=1;i<=nl;i++) print "  " locs[i]; if(over["l"]) print "  ... (+" over["l"] " more)" }
}
'
