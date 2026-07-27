# throw of numeric/boolean/null/undefined literal — match anywhere on line,
# strip line comments and inline /* */ blocks first so `// throw 5` style
# annotations and example code in comments don't trigger.
THROW_LITERAL=$(awk '
  {
    line = $0
    # Strip /* ... */ blocks on a single line (heuristic; multi-line not handled).
    # Greedy on purpose: a line with TWO inline blocks sandwiching real code
    # (`/* a */ throw 5; /* b */`) erases the `throw 5` and misses it. Vanishingly
    # rare; accepted over a count_code_lines-style state machine here.
    gsub(/\/\*.*\*\//, "", line)
    # Strip // line comments (heuristic — does not preserve `//` inside a string,
    # which is rare in real code).
    sub(/\/\/.*$/, "", line)
    if (match(line, /(^|[^a-zA-Z_$])throw[ \t]+(-?[0-9]+(\.[0-9]+)?|null|undefined|true|false)([ \t]|;|}|$)/)) {
      print NR ": " $0
    }
  }
' "$FILE_PATH" 2>/dev/null | head -3)
if [[ -n "$THROW_LITERAL" ]]; then
  add_error "throw of non-Error literal in $FILE_PATH — throw an Error (or subclass) instead"$'\n'"${THROW_LITERAL}"
fi

