PY_MAX_FN=$(python_max_fn_lines)
# Indent-based span (Python has no braces): a `def `/`async def ` line opens a
# span at its OWN indent; the span closes at the next nonblank line whose
# indent is <= the def's (decorators above are never counted — they sit at
# the same indent as the def and simply close the PRIOR span before the def
# line opens a new one). Only code lines count (blanks and full-line `#`
# comments are skipped), matching count_python_code_lines. Known limitation:
# a line continuation or multi-line string containing a dedented look-alike
# line is not tokenized here — same heuristic class as the rust brace counter.
LONG_PY_FUNCS=$(awk -v max="$PY_MAX_FN" '
  function lead(s,   i,c,n) {
    n = 0
    for (i = 1; i <= length(s); i++) {
      c = substr(s, i, 1)
      if (c == " " || c == "\t") n++
      else break
    }
    return n
  }
  {
    raw = $0
    trimmed = raw
    sub(/^[ \t]+/, "", trimmed)
    blank = (length(trimmed) == 0)
    if (infn && !blank) {
      if (lead(raw) <= def_indent) {
        if (count > max) printf "%s:%d (%d lines)\n", name, start, count
        infn = 0
      }
    }
    if (!infn && raw ~ /^[ \t]*(async[ \t]+)?def[ \t]/) {
      infn = 1
      def_indent = lead(raw)
      start = NR
      match(trimmed, /def[ \t]+[A-Za-z_][A-Za-z0-9_]*/)
      name = substr(trimmed, RSTART, RLENGTH)
      sub(/^def[ \t]+/, "", name)
      count = 0
    }
    if (infn && !blank && substr(trimmed, 1, 1) != "#") count++
  }
  END {
    if (infn && count > max) printf "%s:%d (%d lines)\n", name, start, count
  }
' "$FILE_PATH" 2>/dev/null)
if [[ -n "$LONG_PY_FUNCS" ]]; then
  add_error "Function too long in $FILE_PATH (>${PY_MAX_FN} lines) — extract helpers."$'\n'"${LONG_PY_FUNCS}"
fi

