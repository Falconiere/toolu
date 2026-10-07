use std::path::{Path, PathBuf};

use toolu_engine::babysit::TickRequest;
use toolu_epic_orchestrator::babysit::{Next, next};
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{BABYSIT_TICK, STATUS_SNAPSHOT, epic, statusline};

#[test]
fn the_epic_engine_runs_the_injected_pr_babysit_tick() {
  let request = TickRequest {
    repo: "Falconiere/toolu".into(),
    number: 460,
    state_file: PathBuf::from("/state/460.json"),
    now: None,
  };
  assert_eq!(
    next(BABYSIT_TICK, &request),
    Next::Attention("babysit tick for Falconiere/toolu#460 failed: not ported yet (#433)".into())
  );
}

#[test]
fn statusline_gets_the_hubs_status_snapshot() {
  let roots = Roots::new(Env::default(), None);
  let document = STATUS_SNAPSHOT
    .snapshot(&roots, Path::new("/repo"))
    .expect("ported snapshot");
  assert_eq!(document["namespace"], "status");
}

#[test]
fn the_linked_namespaces_keep_their_placeholder_output() {
  let epic_matches = toolu_epic_orchestrator::command()
    .try_get_matches_from(["epic", "planned"])
    .expect("epic planned matches");
  let epic_out = epic(&epic_matches, &Ctx::default())
    .stdout
    .unwrap_or_default();
  assert!(
    epic_out.starts_with("toolu epic is not ported yet (#434, #435, #448)"),
    "{epic_out}"
  );
  let status_matches = toolu_statusline::command()
    .try_get_matches_from(["statusline", "planned"])
    .expect("statusline planned matches");
  let status_out = statusline(&status_matches, &Ctx::default())
    .stdout
    .unwrap_or_default();
  assert!(
    status_out.starts_with("toolu statusline is not ported yet (#431)"),
    "{status_out}"
  );
}
