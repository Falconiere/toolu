RUST_MAX_FN=$(rust_max_fn_lines)
# Match fn with any leading visibility/qualifier combo: pub, pub(crate)/pub(super),
# async, const, unsafe, extern "C", and combinations thereof.
# The fn end is found with a brace-depth counter from the fn's own line, so a
# method inside an impl/mod is measured to ITS close (not the impl's), and an
# inner if/match/loop close can't end the range early. Body-less trait decls
# (`fn f();`) are released on the `;`. Single-line "..." strings and the
# '{'/'}' char literals are stripped before counting so a lone brace inside
# them cannot skew the depth. Known limitation: a brace inside a MULTI-LINE
# string (incl. raw strings) still leaks into the count — an unbalanced one
# leaves the fn unmeasured (fail-open), never falsely flagged short.
LONG_RS_FUNCS=$(awk -v max="$RUST_MAX_FN" -v q="'" '
  !infn && /^[[:space:]]*(pub(\([^)]+\))?[[:space:]]+)?((async|const|unsafe|extern)([[:space:]]+"[^"]*")?[[:space:]]+)*fn / {
    infn=1; start=NR; name=$0; depth=0; opened=0
  }
  infn {
    line=$0
    gsub(/\\"/, "", line)            # escaped quotes would derail the span strip
    gsub(/"[^"]*"/, "", line)        # single-line string contents
    gsub(q "[{}]" q, "", line)       # brace char literals (lifetimes never contain braces)
    no=gsub(/\{/, "{", line); nc=gsub(/\}/, "}", line)
    depth += no - nc
    if (no > 0) opened=1
    if (opened && depth <= 0) {
      len=NR-start
      if (len > max) printf "%s:%d (%d lines)\n", name, start, len
      infn=0
    }
    if (!opened && $0 ~ /;[[:space:]]*$/) infn=0
  }
' "$FILE_PATH" 2>/dev/null)
# Test files are exempt: a table-driven test is one long fn by nature, and the
# toolu-conventions Rust test header prescribes `clippy::too_many_lines` for
# exactly that reason — flagging it here would contradict the template.
if [[ -n "$LONG_RS_FUNCS" && ! "$_is_rust_test" -eq 1 ]]; then
  add_error "Function too long in $FILE_PATH (>${RUST_MAX_FN} lines) — extract helpers."
fi

