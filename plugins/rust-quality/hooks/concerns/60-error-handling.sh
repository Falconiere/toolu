# shellcheck shell=bash
# --- Error-handling rules (zero tolerance) ---
# src/ is production code, so any panic-on-error pattern here is a prod panic
# — EXCEPT in a test file. "tests must live in tests/" is true of the crate-root
# suite but not of a module-sibling one: a crate that bans mod.rs wires its
# colocated tests as src/<mod>/tests/<name>.rs, which is under src/ and is still
# test code. `.unwrap()` there is how a test reports a failed assertion, not a
# production panic. Same `_is_rust_test` gate 90-docs.sh already uses (set in
# 20-tests.sh: *_test.rs/*_tests.rs filename, or a #[test]/#[rstest]/#[test_case]
# attribute in the body).
if [[ "$FILE_PATH" == */src/* && "$_is_rust_test" -ne 1 ]] && command -v ast-grep >/dev/null 2>&1; then
  # ONE ast-grep process for all six rules. `ast-grep run -p` re-parses the file
  # per pattern; `scan --inline-rules` parses once and returns every rule's hits
  # as JSON. The excerpt lines rebuilt from .file/.range/.lines below are
  # byte-identical to what `run -p` printed. Mirrors ts-quality/78-error-ast.sh.
  #
  # severity is `warning` on purpose: `scan` exits non-zero when an ERROR-level
  # rule matches, which would be indistinguishable from a crashed tool. At
  # warning level both a clean and a matching run exit 0, so a non-zero exit (or
  # any stderr) still means the tool genuinely broke.
  ast_err_file="$(mktemp)"
  ast_rules=$(cat <<'RS_AST_RULES'
id: unwrap
language: rust
severity: warning
message: unwrap on Result/Option
rule:
  pattern: '$E.unwrap()'
---
id: expect
language: rust
severity: warning
message: expect on Result/Option
rule:
  pattern: '$E.expect($M)'
---
id: panic
language: rust
severity: warning
message: panic!
rule:
  pattern: 'panic!($$$)'
---
id: todo
language: rust
severity: warning
message: todo!
rule:
  pattern: 'todo!($$$)'
---
id: unimplemented
language: rust
severity: warning
message: unimplemented!
rule:
  pattern: 'unimplemented!($$$)'
---
id: unreachable
language: rust
severity: warning
message: unreachable!
rule:
  pattern: 'unreachable!($$$)'
RS_AST_RULES
)

  ast_grep_failed=0
  ast_grep_fail_detail=""
  ast_lines=""
  ast_json=$(ast-grep scan --inline-rules "$ast_rules" --json "$FILE_PATH" 2>"$ast_err_file")
  ast_rc=$?
  # Flatten every hit to "<ruleId>\t<file>:<line>:<source line>", one output line
  # per matched source line.
  #
  # Two distinct failure modes, reported distinctly: ast-grep itself broke, or it
  # exited 0 and returned something that is not the documented JSON array. Both
  # mean the rules could not be verified, but collapsing them into one message
  # sends the reader to the wrong tool.
  ast_fail_stage=""
  if [[ "$ast_rc" -ne 0 || -s "$ast_err_file" ]]; then
    ast_grep_failed=1
    ast_fail_stage="ast-grep"
  elif ! ast_lines=$(printf '%s' "$ast_json" | jq -r '
        .[] | . as $m
        | ((($m.lines // "") | split("\n")) | to_entries[])
        | "\($m.ruleId)\t\($m.file):\($m.range.start.line + 1 + .key):\(.value)"
      ' 2>/dev/null); then
    ast_grep_failed=1
    ast_fail_stage="jq"
    ast_lines=""
  fi
  if [[ "$ast_grep_failed" -ne 0 ]]; then
    # Exit code plus the first stderr line (capped at ~200 chars) so the surfaced
    # message tells the agent WHAT broke, not just THAT it broke.
    ast_stderr_first=$(head -n 1 "$ast_err_file" 2>/dev/null | cut -c1-200)
    if [[ "$ast_fail_stage" == "jq" ]]; then
      ast_grep_fail_detail="ast-grep exited 0 but its output did not parse as the documented JSON array"
    else
      ast_grep_fail_detail="ast-grep exit ${ast_rc}${ast_stderr_first:+: $ast_stderr_first}"
    fi
  fi

  # join_hits BLOCK... — concatenate non-empty hit blocks into $JOINED with one
  # newline between them. Each block comes from a $(...) capture, which strips the
  # trailing newline, so plain concatenation glued one block's last line onto the
  # next block's first line. Sets a variable instead of printing to avoid a fork
  # on a path that runs after every edit.
  join_hits() {
    local _part
    JOINED=""
    for _part in "$@"; do
      [ -z "$_part" ] && continue
      [ -n "$JOINED" ] && JOINED="${JOINED}"$'\n'
      JOINED="${JOINED}${_part}"
    done
  }

  # ast_hits RULE_ID LIMIT — this rule's excerpt lines, capped like before.
  ast_hits() {
    local _id="$1" _limit="$2" _row _rest
    [ -n "$ast_lines" ] || return 0
    while IFS=$'\t' read -r _row _rest; do
      [ "$_row" = "$_id" ] && printf '%s\n' "$_rest"
    done <<< "$ast_lines" | head -n "$_limit"
  }

  UNWRAP_HITS=$(ast_hits unwrap 5)
  if [[ -n "$UNWRAP_HITS" ]]; then
    add_error ".unwrap() in $FILE_PATH — use ? or match on Result/Option"$'\n'"${UNWRAP_HITS}"
  fi
  EXPECT_HITS=$(ast_hits expect 5)
  if [[ -n "$EXPECT_HITS" ]]; then
    add_error ".expect() in $FILE_PATH — use ? or match on Result/Option"$'\n'"${EXPECT_HITS}"
  fi

  PANIC_HITS=$(ast_hits panic 3)
  TODO_HITS=$(ast_hits todo 3)
  UNIMPL_HITS=$(ast_hits unimplemented 3)
  UNREACH_HITS=$(ast_hits unreachable 3)
  if [[ -n "$PANIC_HITS" || -n "$TODO_HITS" || -n "$UNIMPL_HITS" || -n "$UNREACH_HITS" ]]; then
    join_hits "$PANIC_HITS" "$TODO_HITS" "$UNIMPL_HITS" "$UNREACH_HITS"
    add_error "panic!/todo!/unimplemented!/unreachable! in $FILE_PATH — return a Result instead"$'\n'"${JOINED}"
  fi

  rm -f "$ast_err_file"
  if [[ "$ast_grep_failed" -ne 0 ]]; then
    add_error "ast-grep failed while scanning $FILE_PATH — ${ast_grep_fail_detail:-unknown error}; error-handling rules could not be verified. Fix the tool/file and re-edit"
  fi
fi
