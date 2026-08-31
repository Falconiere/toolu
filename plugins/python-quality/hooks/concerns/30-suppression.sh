
# Forbidden suppression markers — fix the underlying issue in code, never
# silence the tool. Four zero-false-positive forms, one awk pass (mirrors
# 40-unsafe.sh's single-process style). No \b anywhere — BSD awk (macOS) has
# none; explicit boundary characters are used instead:
#   - bare `except:`             (no exception type — swallows EVERYTHING,
#                                  including KeyboardInterrupt/SystemExit)
#   - one-line `except ...: pass` (catches and silently discards)
#   - blanket `# noqa`           (no `:CODE` suffix — suppresses every rule on
#                                  the line, not a scoped `# noqa: E501`)
#   - blanket `# type: ignore`   (no `[code]` suffix — suppresses every mypy
#                                  error on the line, not a scoped
#                                  `# type: ignore[arg-type]`)
# Scoped forms are distinguished from blanket ones by a SECOND regex on the
# same line rather than a lookahead (POSIX/BSD awk has none).
SUPPRESSION_HITS=$(awk '
  /^[[:space:]]*except[[:space:]]*:[[:space:]]*(#.*)?$/ {
    print NR": bare except: (swallows everything)"; next
  }
  /^[[:space:]]*except[^:]*:[[:space:]]*pass[[:space:]]*(#.*)?$/ {
    print NR": one-line except ...: pass (silently discarded)"; next
  }
  /#[ \t]*noqa/ && !/#[ \t]*noqa[ \t]*:[ \t]*[A-Za-z0-9]/ {
    print NR": blanket # noqa (no :CODE suffix)"; next
  }
  /#[ \t]*type:[ \t]*ignore/ && !/#[ \t]*type:[ \t]*ignore\[/ {
    print NR": blanket # type: ignore (no [code] suffix)"; next
  }
' "$FILE_PATH" 2>/dev/null | head -5)
if [[ -n "$SUPPRESSION_HITS" ]]; then
  add_error "Forbidden suppression in $FILE_PATH — remove it and fix the underlying issue. Scoped forms (except SpecificError:, # noqa: E501, # type: ignore[arg-type]) are fine; blanket ones are not:"$'\n'"${SUPPRESSION_HITS}"
fi

