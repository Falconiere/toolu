use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

fn matches(args: &[&str]) -> clap::ArgMatches {
  command().try_get_matches_from(args).unwrap()
}

#[test]
fn the_code_review_crate_is_its_plugin() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(command().get_name(), PLUGIN);
}

#[test]
fn run_fails_closed_as_unconfigured_with_a_json_report() {
  let ctx = Ctx {
    json: true,
    ..Ctx::default()
  };
  let outcome = run(&matches(&["code-review", "run", "--effort", "xhigh"]), &ctx);
  assert_eq!(outcome.exit, Exit::Unavailable);
  assert_eq!(outcome.stderr, None);
  let doc: serde_json::Value = serde_json::from_str(outcome.stdout.as_deref().unwrap()).unwrap();
  assert_eq!(doc["verdict"], "error");
  assert_eq!(doc["effort"], "xhigh");
  assert_eq!(doc["error"]["kind"], "unconfigured");
  assert_eq!(doc["findings"], serde_json::json!([]));
}

#[test]
fn run_without_json_says_why_on_stderr() {
  let outcome = run(&matches(&["code-review", "run"]), &Ctx::default());
  assert_eq!(outcome.exit, Exit::Unavailable);
  assert_eq!(outcome.stdout, None);
  let line = outcome.stderr.unwrap();
  assert!(line.starts_with("code-review: unconfigured: "), "{line}");
  assert!(line.contains("nothing was sent"), "{line}");
}

#[test]
fn a_report_without_an_error_prints_its_verdict_line() {
  let mut report =
    super::Report::failed(super::budget::Effort::Small, super::ErrorKind::Outage, "x");
  report.verdict = super::contract::Verdict::Approved;
  report.error = None;
  let outcome = super::present(&report, &Ctx::default());
  assert_eq!(outcome.exit, Exit::Success);
  assert_eq!(
    outcome.stdout.as_deref(),
    Some("code-review: approved, 0 blocking finding(s)")
  );
}
