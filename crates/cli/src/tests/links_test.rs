use std::path::{Path, PathBuf};

use toolu_engine::babysit::TickRequest;
use toolu_epic_orchestrator::babysit::{Next, next};
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{BABYSIT_TICK, STATUS_SNAPSHOT, epic};

#[test]
fn the_epic_engine_runs_the_injected_pr_babysit_tick() {
  let request = TickRequest {
    repo: "Falconiere/toolu".into(),
    number: 460,
    state_file: PathBuf::from("/state/460.json"),
    now: Some("soon".into()),
  };
  assert_eq!(
    next(BABYSIT_TICK, &request),
    Next::Attention(
      "babysit tick for Falconiere/toolu#460 failed: usage: --now must be YYYY-MM-DDTHH:MM:SSZ"
        .into()
    )
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
fn epic_runs_its_verbs_through_the_link() {
  let epic_matches = toolu_epic_orchestrator::command()
    .try_get_matches_from(["epic", "gate", "--pr", "nonsense"])
    .expect("epic gate matches");
  let outcome = epic(&epic_matches, &Ctx::default());
  assert_eq!(
    outcome.exit,
    toolu_protocol::exit::Exit::Usage,
    "{outcome:?}"
  );
}
