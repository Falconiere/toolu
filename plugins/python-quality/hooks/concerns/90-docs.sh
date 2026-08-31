# --- Docs (soft advisory, never blocks) ---
# A top-level, non-underscore-prefixed `def`/`class` should open with a
# docstring. Advisory only — collected separately from MESSAGES so it never
# sets the failing gate. Column-0 (top-level) only: indented defs/methods are
# exempt (noise control — every private helper method would otherwise need
# one). The signature may span multiple lines; it is tracked forward to the
# line that actually ends with `:` before the docstring check runs. Known
# limitation: a `def`/`class`-shaped line quoted inside a module docstring at
# column 0 would be mis-read as a real definition — same class of heuristic
# limit as the other line-based scans in this plugin.
DOC_ADVISORY=""
_undoc=$(awk -v sq="'" '
  function isblank(s,    t) { t = s; gsub(/^[ \t]+|[ \t]+$/, "", t); return (length(t) == 0) }
  function stripcomment(s,    t) { t = s; sub(/[ \t]#.*$/, "", t); return t }
  function endswithcolon(s,    t) {
    t = stripcomment(s); gsub(/[ \t]+$/, "", t)
    return (length(t) > 0 && substr(t, length(t), 1) == ":")
  }
  function opens_docstring(s,    t, p, body) {
    t = s; gsub(/^[ \t]+/, "", t)
    p = 0
    if (t ~ /^[rRfF]/) p = 1
    body = substr(t, p + 1, 3)
    return (body == "\"\"\"" || body == sq sq sq)
  }
  {
    line = $0
    if (waitbody) {
      if (isblank(line)) next
      if (!opens_docstring(line)) printf "%d: %s\n", defline, defname
      waitbody = 0; next
    }
    if (insig) {
      if (endswithcolon(line)) { insig = 0; waitbody = 1 }
      next
    }
    if (line ~ /^(async[ \t]+def|def|class)[ \t]+[A-Za-z_][A-Za-z0-9_]*/) {
      tmp = line
      sub(/^(async[ \t]+def|def|class)[ \t]+/, "", tmp)
      match(tmp, /^[A-Za-z_][A-Za-z0-9_]*/)
      name = substr(tmp, RSTART, RLENGTH)
      if (substr(name, 1, 1) != "_") {
        defline = NR; defname = name
        if (endswithcolon(line)) { waitbody = 1 } else { insig = 1 }
      }
    }
  }
' "$FILE_PATH" 2>/dev/null | head -3)
if [[ -n "$_undoc" ]]; then
  DOC_ADVISORY="Public def/class missing a docstring in $FILE_PATH — add a concise one:"$'\n'"${_undoc}"
fi

