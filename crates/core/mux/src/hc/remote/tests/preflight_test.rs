use super::reports;

#[test]
fn an_old_same_version_cli_without_bound_reports_is_not_compatible() {
  let epic = "--run <RUN>  --expected-base <SHA>";
  let fixer = "--run-id <RUN>  --expected-base <SHA>  --commit <SHA>";
  assert!(reports(epic, fixer).is_ok());
  assert!(reports("--status-file <FILE>", fixer).is_err());
  assert!(reports(epic, "--note <TEXT>").is_err());
}
