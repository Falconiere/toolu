# --- Docs (soft advisory, never blocks) ---
# A public API item in src/ should carry a concise /// doc comment. Advisory
# only — collected separately from MESSAGES so it never sets the failing gate.
DOC_ADVISORY=""
if [[ "$FILE_PATH" == */src/* && ! "$_is_rust_test" -eq 1 ]]; then
  _undoc=$(awk '
    /^[[:space:]]*$/   { next }   # blanks do not reset the doc context
    /^[[:space:]]*#\[/ { next }   # attributes sit between doc and item
    {
      if ($0 ~ /^[[:space:]]*pub(\([^)]+\))?[[:space:]]+(fn|struct|enum|trait|const|static|type|mod)[[:space:]]/) {
        if (prev !~ /^[[:space:]]*(\/\/\/|\/\/!)/) printf "%d: %s\n", NR, $0
      }
      prev=$0
    }
  ' "$FILE_PATH" 2>/dev/null | head -3)
  if [[ -n "$_undoc" ]]; then
    DOC_ADVISORY="Public items missing a /// doc comment in $FILE_PATH — add a concise one-line doc:"$'\n'"${_undoc}"
  fi
  # Concise cap: flag doc-comment runs that have grown long.
  _verbose_doc=$(awk '
    /^[[:space:]]*\/\/[\/!]/ { if (run==0) start=NR; run++; next }
    { if (run>12) printf "%d: doc block is %d lines — trim to the essentials\n", start, run; run=0 }
    END { if (run>12) printf "%d: doc block is %d lines — trim to the essentials\n", start, run }
  ' "$FILE_PATH" 2>/dev/null | head -2)
  if [[ -n "$_verbose_doc" ]]; then
    # Separator appended on its own line: ANSI-C quoting is NOT honored inside a
    # ${var:+word} expansion, so it cannot be inlined there.
    [[ -n "$DOC_ADVISORY" ]] && DOC_ADVISORY="${DOC_ADVISORY}"$'\n'
    DOC_ADVISORY="${DOC_ADVISORY}Verbose doc comment in $FILE_PATH — docs must be present but concise:"$'\n'"${_verbose_doc}"
  fi
fi

