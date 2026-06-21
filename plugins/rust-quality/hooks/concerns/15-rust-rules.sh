# shellcheck shell=bash
# --- Canonical structural rules (thin wrapper over lib/rust-rules.sh) ---
# The per-file rule_* functions are the SINGLE source of truth, shared verbatim
# with the repo-wide scanner (define once). This fragment does NOT reimplement
# any check — it just runs each rule on the resolved $FILE_PATH and routes its
# TSV rows (rule<TAB>severity<TAB>file<TAB>line<TAB>autofix<TAB>message) into the
# gate's two channels:
#   block    -> add_error "$msg"        (fails the quality gate)
#   advisory -> DOC_ADVISORY            (non-blocking; 99-finalize.sh emits it)
# This supersedes the old 10-size-file / 20-tests / 50-size-fn / 55-size-impl /
# 90-docs fragments — their logic now lives in the rule library.

# DOC_ADVISORY was previously seeded by 90-docs.sh (now deleted); seed it here so
# 99-finalize.sh's `if [[ -n "$DOC_ADVISORY" ]]` reader stays self-sufficient.
DOC_ADVISORY=""

# Run one rule function and route each TSV row it emits. A blank rule field
# (empty line) is skipped. Process-substitution feeds the loop so add_error /
# DOC_ADVISORY mutate THIS shell, not a pipeline subshell.
_run_rule() {
  # Fields: rule, severity, file, line, autofix, message. The gate keys off
  # rule (non-empty guard) + severity, and surfaces only the message; file,
  # line and autofix are discarded here (they matter to the scanner, not the
  # per-edit gate), hence the three _ placeholders.
  while IFS=$'\t' read -r r s _ _ _ m; do
    [ -n "$r" ] || continue
    if [ "$s" = block ]; then
      add_error "$m"
    else
      DOC_ADVISORY="${DOC_ADVISORY:+$DOC_ADVISORY\n}$m"
    fi
  done < <("$@")
}

for _rr_fn in \
  rule_file_size \
  rule_mod_rs_no_logic \
  rule_generic_name \
  rule_test_location \
  rule_module_doc \
  rule_fn_size \
  rule_impl_size \
  rule_layering_file; do
  _run_rule "$_rr_fn" "$FILE_PATH"
done
