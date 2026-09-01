RUST_MAX_IMPL=$(rust_max_impl_lines)
# Brace-depth counter (same technique + string/char strip as the fn check):
# a bare `^\}` close-marker would be tripped early by a column-0 `}` from
# macro-generated or wide-rustfmt code inside the impl, undercounting it.
LONG_IMPL=$(awk -v max="$RUST_MAX_IMPL" -v q="'" '
  !inimpl && /^[[:space:]]*(unsafe[[:space:]]+)?impl[[:space:]<]/ {
    inimpl=1; start=NR; name=$0; depth=0; opened=0
  }
  inimpl {
    line=$0
    gsub(/\\"/, "", line)
    gsub(/"[^"]*"/, "", line)
    gsub(q "[{}]" q, "", line)
    no=gsub(/\{/, "{", line); nc=gsub(/\}/, "}", line)
    depth += no - nc
    if (no > 0) opened=1
    if (opened && depth <= 0) {
      len=NR-start
      if (len > max) printf "%s:%d (%d lines)\n", name, start, len
      inimpl=0
    }
  }
' "$FILE_PATH" 2>/dev/null)
# Exempt in test files, for the same reason as the fn-length rule above.
if [[ -n "$LONG_IMPL" && "$_is_rust_test" -ne 1 ]]; then
  add_error "Impl block too large in $FILE_PATH (>${RUST_MAX_IMPL} lines) — split into trait impls or modules."
fi

