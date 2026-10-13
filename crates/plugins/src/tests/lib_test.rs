use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{command, run};

#[test]
fn an_unreadable_catalog_fails_naming_the_file() {
  let argv = ["plugins", "list", "--catalog", "/no/such/marketplace.json"];
  let Ok(matches) = command().try_get_matches_from(argv) else {
    panic!("the command line did not parse");
  };
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(outcome.exit, Exit::Failure);
  assert_eq!(
    outcome.stderr.as_deref(),
    Some("toolu: cannot read marketplace manifest: /no/such/marketplace.json")
  );
}

#[test]
fn a_report_prints_text_or_one_json_document_and_failures_exit_1() {
  use serde_json::json;

  use super::outcome;
  use crate::dispatch::Report;
  use crate::fail::Fail;

  let report = Report { text: "  + jev              installed\n".to_owned(), json: json!({"ok": 1}), failed: false };
  let text = outcome(Ok(report.clone()), &Ctx::default());
  assert_eq!((text.exit, text.stdout.as_deref()), (Exit::Success, Some("  + jev              installed")));
  let empty = outcome(Ok(Report { text: String::new(), ..report.clone() }), &Ctx::default());
  assert_eq!(empty.stdout, None);
  let json = outcome(Ok(Report { failed: true, ..report }), &Ctx { json: true, ..Ctx::default() });
  assert_eq!((json.exit, json.stdout.as_deref()), (Exit::Failure, Some("{\"ok\":1}")));
  let usage = outcome(Err(Fail::usage("several hosts found (claude, codex). Choose one with --host.")), &Ctx::default());
  assert_eq!(usage.exit.code(), 64);
  assert_eq!(usage.stderr.as_deref(), Some("toolu: several hosts found (claude, codex). Choose one with --host."));
}
