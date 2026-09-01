_has_inline_cfg_test=0
# Match bare `#[cfg(test)]` and `test` as the FIRST predicate of an all()/any()
# combinator (`#[cfg(all(test, feature = "x"))]`, `#[cfg(any(test, ...))]`).
# Deliberately NOT a loose `cfg(.*\btest\b`: that would false-positive on
# `#[cfg(not(test))]` (the opposite gate) and `#[cfg(feature = "test-utils")]`
# (test inside a string). A non-leading `test` predicate (`all(feature, test)`)
# is the rare miss we accept to keep zero false positives.
#
# EXEMPTION: a bare `#[cfg(test)]` that wires a BODYLESS `mod <name>;` decl —
# single-line (`#[cfg(test)] mod tests;`) or rustfmt's attribute-on-its-own-line
# two-line form (`#[cfg(test)]` then `mod tests;`) — is a pointer to an external
# module-sibling tests/ dir, not inline test code, and is not flagged. Further
# OUTER attributes may sit between the two: `#[path = "tests/stats.rs"]` is the
# only wiring a crate that bans mod.rs has for a module-sibling test file, so
# the decl is looked for past any attribute run, bounded so it cannot scan the
# whole file. Inner attributes (`#![...]`) are deliberately not skipped — they
# cannot precede a declaration. Only the BARE cfg(test) form earns this: the
# all()/any() combinator stays blocked unconditionally (even ahead of a bodyless
# decl), and any body (`mod tests {`, any line form) stays blocked. grep is
# line-based, so the multi-line pairing needs awk (getline the next lines); no
# \b — BSD awk has none, use a negated [[:alnum:]_] class (or line end) for the
# boundary instead.
if [[ "$FILE_PATH" == */src/* ]] \
   && awk '
       /^[[:space:]]*#\[cfg\((all|any)\(test([^[:alnum:]_]|$)/ { found=1; exit }
       /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;[[:space:]]*(\/\/.*)?$/ { next }
       /^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$/ {
         nxt=""
         for (i = 0; i < 8; i++) {
           if ((getline nxt) <= 0) { found=1; exit }
           if (nxt !~ /^[[:space:]]*#\[[^!]/) break
         }
         if (nxt ~ /^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;[[:space:]]*(\/\/.*)?$/) { next }
         found=1; exit
       }
       /^[[:space:]]*#\[cfg\(test\)\]/ { found=1; exit }
       END { exit (found ? 0 : 1) }
     ' "$FILE_PATH" 2>/dev/null; then
  add_error "Inline #[cfg(test)] in $FILE_PATH — unit tests belong in a module-sibling tests/ dir wired by a bodyless #[cfg(test)] mod tests; decl; crate-root tests/ is for integration tests"
  _has_inline_cfg_test=1
fi

# Test placement: a test-bearing .rs file must live under a tests/ dir, kept
# flat (only fixtures/helpers/common subdirs allowed — common/mod.rs is the
# cargo idiom for shared test helpers). Skip when the inline-#[cfg(test)] rule
# already fired: that's the canonical `src/lib.rs` with `#[cfg(test)] mod tests`
# pattern, where "move the file to tests/" is wrong (it would orphan the pub
# items) — the cfg(test) message already says to extract the tests.
_is_rust_test=0
case "$(basename "$FILE_PATH")" in
  *_test.rs|*_tests.rs) _is_rust_test=1 ;;
esac
# #[bench] and #[wasm_bindgen_test] are deliberately NOT in the alternation:
# benches belong in benches/ (cargo convention) and wasm-bindgen tests have no
# single canonical home — "move to tests/" would be a false positive for both.
# Generalized over `#[<any::path>::test]` rather than a hardcoded runtime list,
# so test_log::test, trace_test::test, actix_web::test, etc. all trip the rule;
# `test_case` (the test_case crate's generator) is added explicitly. `#[rstest]`
# is its own alternation (the macro name is not `test`). `#[cfg(test)]` is NOT
# matched here — no `test`/`test_case` token follows `#[` — and stays owned by
# the inline-cfg(test) rule above.
if [[ "$_is_rust_test" -eq 0 ]] \
   && grep -qE '^[[:space:]]*#\[([A-Za-z_][A-Za-z0-9_]*::)*(test|test_case)\b|^[[:space:]]*#\[rstest\b' "$FILE_PATH" 2>/dev/null; then
  _is_rust_test=1
fi
if [[ "$_is_rust_test" -eq 1 && "$_has_inline_cfg_test" -eq 0 ]]; then
  if [[ "$FILE_PATH" != */tests/* ]]; then
    add_error "Rust test file outside tests/: $FILE_PATH — move to a sibling tests/ directory"
  else
    _after_tests="${FILE_PATH##*/tests/}"
    if [[ "$_after_tests" == */* ]]; then
      _subdir="${_after_tests%%/*}"
      if [[ "$_subdir" != "fixtures" && "$_subdir" != "helpers" && "$_subdir" != "common" ]]; then
        add_error "Rust test nested in tests/ subdirectory: $FILE_PATH — keep tests/ flat (only fixtures/helpers/common subdirs allowed)"
      fi
    fi
  fi
fi
