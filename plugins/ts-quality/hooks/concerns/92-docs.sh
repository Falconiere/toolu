# --- Docs (soft advisory, never blocks) ---
# Exported API should carry a concise JSDoc. Advisory only — kept out of
# MESSAGES so it never sets the failing gate. Skips tests, barrels, declarations.
DOC_ADVISORY=""
_ts_base="$(basename "$FILE_PATH")"
if [[ ! "$FILE_PATH" =~ \.(test|spec)\.(ts|tsx)$ && ! "$FILE_PATH" =~ \.d\.ts$ \
      && "$_ts_base" != "index.ts" && "$_ts_base" != "index.tsx" ]]; then
  _undoc=$(awk '
    /^[[:space:]]*$/   { next }   # blanks do not reset the doc context
    /^[[:space:]]*\/\// { next }  # line comments / pragmas sit between doc and export
    {
      if ($0 ~ /^export (async )?function / || $0 ~ /^export (abstract )?class / \
          || $0 ~ /^export default / || $0 ~ /^export (const|interface|type|enum) [A-Z]/ \
          || $0 ~ /^export const [a-z_][A-Za-z0-9_]* = (async )?(\(|function)/) {
        if (prev !~ /\*\/[[:space:]]*$/ && prev !~ /^[[:space:]]*\/\*\*/) printf "%d: %s\n", NR, $0
      }
      prev=$0
    }
  ' "$FILE_PATH" 2>/dev/null | head -3)
  if [[ -n "$_undoc" ]]; then
    DOC_ADVISORY="Exported API missing a JSDoc in $FILE_PATH — add a concise /** */ doc:"$'\n'"${_undoc}"
  fi
  _verbose_doc=$(awk '
    !inb && /\/\*\*/ { inb=1; start=NR; cnt=0 }   # !inb: a /** in prose must not reset the count mid-block
    inb { cnt++ }
    inb && /\*\// { if (cnt>12) printf "%d: JSDoc block is %d lines — trim to the essentials\n", start, cnt; inb=0 }
  ' "$FILE_PATH" 2>/dev/null | head -2)
  if [[ -n "$_verbose_doc" ]]; then
    # Separator appended on its own line: ANSI-C quoting is NOT honored inside a
    # ${var:+word} expansion, so the old one-liner would have emitted a literal
    # $'\n' there.
    [[ -n "$DOC_ADVISORY" ]] && DOC_ADVISORY="${DOC_ADVISORY}"$'\n'
    DOC_ADVISORY="${DOC_ADVISORY}Verbose JSDoc in $FILE_PATH — docs must be present but concise:"$'\n'"${_verbose_doc}"
  fi
fi

