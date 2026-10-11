use std::path::{Path, PathBuf};

use toolu_engine::LinkError;
use toolu_engine::babysit::{BabysitTick, TickRequest};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{PLUGIN, Tick, command, needs_stdin, run};

#[test]
fn the_pr_babysit_crate_is_its_plugin_and_runs_its_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command()
    .try_get_matches_from(["babysit", "parse-verdict"])
    .unwrap();
  assert!(needs_stdin(&matches));
  let ctx = Ctx {
    stdin: Some(String::new()),
    ..Ctx::default()
  };
  let outcome = run(&matches, &ctx);
  assert_eq!(outcome.exit, Exit::Success);
  let parsed: serde_json::Value = serde_json::from_str(outcome.stdout.as_deref().unwrap()).unwrap();
  assert_eq!(parsed, crate::verdict::parse(""));
  let fixer = command()
    .try_get_matches_from(["babysit", "fixer-report", "r", "done"])
    .unwrap();
  assert!(!needs_stdin(&fixer));
}

#[test]
fn the_engine_tick_runs_the_port_and_reports_its_failure() {
  let tick: &dyn BabysitTick = &Tick;
  let request = TickRequest {
    repo: "Falconiere/toolu".into(),
    number: 460,
    state_file: PathBuf::from("/state/460.json"),
    now: Some("soon".into()),
  };
  assert_eq!(
    tick.tick(&request),
    Err(LinkError::Failed(
      "usage: --now must be YYYY-MM-DDTHH:MM:SSZ".into()
    ))
  );
}
