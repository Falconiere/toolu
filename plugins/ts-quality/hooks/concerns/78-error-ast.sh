# shellcheck shell=bash
# --- Error-handling rules (zero tolerance) ---

if command -v ast-grep >/dev/null 2>&1; then
  # ONE ast-grep process for every rule below. The previous shape ran
  # `ast-grep run -p <pattern>` once per pattern — 14 spawns, each re-parsing the
  # same file — and measured ~170ms of the TS gate's ~300ms on every edit.
  # `scan --inline-rules` parses once and returns every rule's hits as JSON
  # (~13ms). The excerpt lines rebuilt from .file/.range/.lines below are
  # byte-identical to what `run -p` printed, so message bodies are unchanged.
  #
  # severity is `warning` on purpose: `scan` exits non-zero when an ERROR-level
  # rule matches, which would be indistinguishable from a crashed tool. At
  # warning level both a clean and a matching run exit 0, so a non-zero exit (or
  # any stderr) still means the tool genuinely broke — preserving the fail-loud
  # contract the old per-pattern ast_scan had.
  ts_ast_err_file="$(mktemp)"
  ts_ast_rules=$(cat <<'TS_AST_RULES'
id: empty-catch
language: ts
severity: warning
message: empty catch
rule:
  pattern: 'try { $$$ } catch ($_) { }'
---
id: empty-catch-noarg
language: ts
severity: warning
message: empty catch (no binding)
rule:
  pattern: 'try { $$$ } catch { }'
---
id: empty-catch-handler
language: ts
severity: warning
message: empty promise catch handler
rule:
  pattern: '$_.catch(() => { })'
---
id: null-catch-handler
language: ts
severity: warning
message: promise catch handler returning null
rule:
  pattern: '$_.catch(() => null)'
---
id: undef-catch-handler
language: ts
severity: warning
message: promise catch handler returning undefined
rule:
  pattern: '$_.catch(() => undefined)'
---
id: swallow-null-arg
language: ts
severity: warning
message: catch returns null
rule:
  pattern: 'try { $$$ } catch ($_) { return null }'
---
id: swallow-undef-arg
language: ts
severity: warning
message: catch returns undefined
rule:
  pattern: 'try { $$$ } catch ($_) { return undefined }'
---
id: swallow-null
language: ts
severity: warning
message: catch returns null (no binding)
rule:
  pattern: 'try { $$$ } catch { return null }'
---
id: swallow-undef
language: ts
severity: warning
message: catch returns undefined (no binding)
rule:
  pattern: 'try { $$$ } catch { return undefined }'
---
# A bare `return` in an ast-grep pattern acts as a WILDCARD over the optional
# argument: `catch { return }` also matched `return []`, `return 42` and
# `return null`, i.e. every non-nullish fallback got reported as "returns a
# nullish value". Match the catch clause itself, then require that its single
# return statement carries no argument at all.
id: swallow-bare
language: ts
severity: warning
message: catch returns nothing
rule:
  all:
    - any:
        - pattern:
            context: 'try {} catch { return }'
            selector: catch_clause
        - pattern:
            context: 'try {} catch ($_) { return }'
            selector: catch_clause
    - not:
        has:
          stopBy: end
          kind: return_statement
          has:
            stopBy: neighbor
            pattern: $X
---
id: throw-empty-error
language: ts
severity: warning
message: throw new Error() with no message
rule:
  pattern: 'throw new Error()'
---
id: throw-string
language: ts
severity: warning
message: throw of a string literal
rule:
  pattern: 'throw "$S"'
---
id: throw-template
language: ts
severity: warning
message: throw of a template literal
rule:
  pattern: 'throw `$S`'
TS_AST_RULES
)

  ts_ast_failed=0
  ts_ast_fail_detail=""
  ts_ast_lines=""
  ts_ast_json=$(ast-grep scan --inline-rules "$ts_ast_rules" --json "$FILE_PATH" 2>"$ts_ast_err_file")
  ts_ast_rc=$?
  # Flatten every hit to "<ruleId>\t<file>:<line>:<source line>", one output line
  # per matched source line — the exact shape `run -p` emitted, now tagged with
  # the rule that produced it. A jq failure here means ast-grep returned
  # something that is not the documented JSON array, which is a tool failure too.
  if [[ "$ts_ast_rc" -ne 0 || -s "$ts_ast_err_file" ]]; then
    ts_ast_failed=1
  elif ! ts_ast_lines=$(printf '%s' "$ts_ast_json" | jq -r '
        .[] | . as $m
        | ((($m.lines // "") | split("\n")) | to_entries[])
        | "\($m.ruleId)\t\($m.file):\($m.range.start.line + 1 + .key):\(.value)"
      ' 2>/dev/null); then
    ts_ast_failed=1
    ts_ast_lines=""
  fi
  if [[ "$ts_ast_failed" -ne 0 ]]; then
    # Same diagnostic the per-pattern scanner surfaced: exit code plus the first
    # stderr line (capped) so the agent learns WHAT broke, not just THAT it did.
    ts_ast_stderr_first=$(head -n 1 "$ts_ast_err_file" 2>/dev/null | cut -c1-200)
    ts_ast_fail_detail="exit ${ts_ast_rc}${ts_ast_stderr_first:+: $ts_ast_stderr_first}"
  fi

  # ts_join_hits BLOCK... — concatenate non-empty hit blocks into $TS_JOINED with
  # one newline between them. Each block comes from a $(...) capture, which strips
  # the trailing newline, so plain concatenation glued one block's last line onto
  # the next block's first line. Sets a variable instead of printing to avoid a
  # fork on a path that runs after every edit.
  ts_join_hits() {
    local _part
    TS_JOINED=""
    for _part in "$@"; do
      [ -z "$_part" ] && continue
      [ -n "$TS_JOINED" ] && TS_JOINED="${TS_JOINED}"$'\n'
      TS_JOINED="${TS_JOINED}${_part}"
    done
  }

  # ts_ast_hits RULE_ID LIMIT — this rule's excerpt lines, capped like before.
  ts_ast_hits() {
    local _id="$1" _limit="$2" _row _rest
    [ -n "$ts_ast_lines" ] || return 0
    while IFS=$'\t' read -r _row _rest; do
      [ "$_row" = "$_id" ] && printf '%s\n' "$_rest"
    done <<< "$ts_ast_lines" | head -n "$_limit"
  }

  # Empty catch — swallowed error
  EMPTY_CATCH=$(ts_ast_hits empty-catch 3)
  EMPTY_CATCH_NOARG=$(ts_ast_hits empty-catch-noarg 3)
  if [[ -n "$EMPTY_CATCH" || -n "$EMPTY_CATCH_NOARG" ]]; then
    ts_join_hits "$EMPTY_CATCH" "$EMPTY_CATCH_NOARG"
    add_error "Empty catch block in $FILE_PATH — handle the error or rethrow; do not swallow"$'\n'"${TS_JOINED}"
  fi

  # Silent promise rejection — .catch(() => {}) / .catch(() => null)
  EMPTY_CATCH_HANDLER=$(ts_ast_hits empty-catch-handler 3)
  NULL_CATCH_HANDLER=$(ts_ast_hits null-catch-handler 3)
  UNDEF_CATCH_HANDLER=$(ts_ast_hits undef-catch-handler 3)
  if [[ -n "$EMPTY_CATCH_HANDLER" || -n "$NULL_CATCH_HANDLER" || -n "$UNDEF_CATCH_HANDLER" ]]; then
    ts_join_hits "$EMPTY_CATCH_HANDLER" "$NULL_CATCH_HANDLER" "$UNDEF_CATCH_HANDLER"
    add_error "Silent promise rejection in $FILE_PATH — log or rethrow the error"$'\n'"${TS_JOINED}"
  fi

  # Swallow via a catch that just returns a nullish value — error vanishes.
  SWALLOW_CATCH=""
  for _rule in swallow-null-arg swallow-undef-arg \
               swallow-null swallow-undef swallow-bare; do
    _h=$(ts_ast_hits "$_rule" 2)
    ts_join_hits "$SWALLOW_CATCH" "$_h"
    SWALLOW_CATCH="$TS_JOINED"
  done
  if [[ -n "$SWALLOW_CATCH" ]]; then
    add_error "Catch swallows the error by returning a nullish value in $FILE_PATH — handle, log, or rethrow it"$'\n'"${SWALLOW_CATCH}"
  fi

  # throw new Error() with no message
  THROW_EMPTY_ERROR=$(ts_ast_hits throw-empty-error 3)
  if [[ -n "$THROW_EMPTY_ERROR" ]]; then
    add_error "throw new Error() with no message in $FILE_PATH — include a descriptive message"$'\n'"${THROW_EMPTY_ERROR}"
  fi

  # throw of string literal — breaks instanceof Error
  THROW_STRING=$(ts_ast_hits throw-string 3)
  THROW_TSTR=$(ts_ast_hits throw-template 3)
  if [[ -n "$THROW_STRING" || -n "$THROW_TSTR" ]]; then
    ts_join_hits "$THROW_STRING" "$THROW_TSTR"
    add_error "throw of string literal in $FILE_PATH — throw an Error (or subclass) instead"$'\n'"${TS_JOINED}"
  fi

  rm -f "$ts_ast_err_file"
  if [[ "$ts_ast_failed" -ne 0 ]]; then
    add_error "ast-grep failed while scanning $FILE_PATH (${ts_ast_fail_detail:-unknown error}) — error-handling rules could not be verified; fix the tool/file and re-edit"
  fi
fi
