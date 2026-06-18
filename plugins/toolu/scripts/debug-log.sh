#!/usr/bin/env bash
# debug-log.sh — language-agnostic log summarizer for the debug skill's Observe step.
# Reads a log (stdin or --file) and emits a compact, capped summary: deduped error/warning
# lines plus a tail of recent lines, always reporting TOTAL lines so the caller knows what was
# elided. Its primary job is cap enforcement — a huge log in, a small summary out, never a flood.
# Output stays at/under DEBUG_MAX_LINES lines AND under DEBUG_MAX_BYTES bytes.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: debug-log.sh [--file <path>] [--json]
  Reads a log from stdin or --file and prints a compact, capped summary:
  deduped ERROR/WARN-ish lines and a tail of the most recent lines.
  --json    Emit a JSON object {errors,tail,total_lines,truncated} instead of text.
Env caps (override): DEBUG_MAX_LINES=100 (max output lines) DEBUG_MAX_BYTES=65536 (hard output byte ceiling)
EOF
}

file=""
json=0
while [ $# -gt 0 ]; do
  case "$1" in
    --file) file="${2:-}"; shift 2 || { echo "debug-log.sh: --file needs a path" >&2; exit 2; } ;;
    --json) json=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "debug-log.sh: unknown arg: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [ -n "$file" ]; then
  [ -r "$file" ] || { echo "debug-log.sh: cannot read file: $file" >&2; exit 2; }
  exec < "$file"
elif [ -t 0 ]; then
  echo "debug-log.sh: no input (pipe a log or pass --file)" >&2
  exit 2
fi

LC_ALL=C awk -v json="$json" \
    -v maxl="${DEBUG_MAX_LINES:-100}" \
    -v maxb="${DEBUG_MAX_BYTES:-65536}" '
function jesc(s){ gsub(/\\/,"\\\\",s); gsub(/"/,"\\\"",s); gsub(/\t/," ",s); gsub(/\r/,"",s); return s }
# Normalize a leading timestamp to a placeholder so timestamped repeats collapse on dedup.
function norm(s,   k){ k=s
  sub(/^[0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9:.]+([Zz]|[+-][0-9:]+)?/,"<TS>",k)  # ISO 8601
  sub(/^\[[0-9]{2}:[0-9]{2}:[0-9]{2}([.,][0-9]+)?\]/,"<TS>",k)                 # [12:00:00]
  return k
}
{
  total=NR
  line=$0; sub(/\r$/,"",line)
  tail[NR]=line                                            # ring of recent lines (indexed by NR)
  if (NR>maxl) delete tail[NR-maxl]                        # keep at most maxl tail candidates
  if (line ~ /[Ee][Rr][Rr][Oo][Rr]|[Ww][Aa][Rr][Nn]|[Pp][Aa][Nn][Ii][Cc]|[Ff][Aa][Ii][Ll]|[Ee][Xx][Cc][Ee][Pp][Tt][Ii][Oo][Nn]|[Ff][Aa][Tt][Aa][Ll]|[Tt][Rr][Aa][Cc][Ee][Bb][Aa][Cc][Kk]/) {
    key=norm(line)
    if (!(key in eseen)) { eseen[key]=1; errs[++ne]=line }
    else eskip++
  }
}
END {
  # Budget: errors get up to ~60% of the line cap, tail gets the rest (header lines count too).
  emax = int(maxl*0.6); if (emax<1) emax=1
  eshow = (ne<emax?ne:emax); eover = ne-eshow
  # Reserve lines already spent by error section + its 2 headers; remainder for tail + its header.
  used = eshow + (ne>0?1:0) + (eover>0?1:0) + 1            # errors + ERRORS hdr + overflow + TOTAL line
  tmax = maxl - used - 1                                   # -1 for the TAIL header
  if (tmax<0) tmax=0
  # Tail = the last tshow of the kept tail lines.
  tn=0; for(i=(total>maxl?total-maxl+1:1); i<=total; i++) if(i in tail) tline[++tn]=tail[i]
  tshow = (tn<tmax?tn:tmax)
  tstart = tn - tshow + 1; if (tstart<1) tstart=1

  if (json) {
    printf "{"
    printf "\"errors\":["; for(i=1;i<=eshow;i++){printf "%s\"%s\"",(i>1?",":""),jesc(errs[i])} printf "],"
    printf "\"tail\":[";   for(i=tstart;i<=tn;i++){printf "%s\"%s\"",(i>tstart?",":""),jesc(tline[i])} printf "],"
    printf "\"total_lines\":%d,", total
    printf "\"truncated\":%s", ((eover>0 || tshow<tn || total>maxl)?"true":"false")
    printf "}\n"
    exit 0
  }

  # Text mode — assemble into a buffer, then enforce the byte ceiling by dropping from the end.
  o=0
  out[++o]="ERRORS/WARNINGS (" eshow (eover>0?"+":"") "):"
  for(i=1;i<=eshow;i++) out[++o]="  " errs[i]
  if (eover>0) out[++o]="  ... (+" eover " more)"
  out[++o]="TAIL (last " tshow " of " total " lines):"
  for(i=tstart;i<=tn;i++) out[++o]="  " tline[i]
  out[++o]="TOTAL lines: " total

  # Hard byte ceiling: count bytes (incl. newline) and drop trailing lines if we would exceed maxb.
  bytes=0; keep=o
  for(i=1;i<=o;i++){ bytes += length(out[i])+1; if (bytes>maxb) { keep=i-1; break } }
  bytetrunc = (keep<o)
  for(i=1;i<=keep;i++) print out[i]
  if (bytetrunc) print "... (output truncated to stay under " maxb " bytes)"
}
'
