use std::path::Path;

use toolu_engine::LinkError;
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{Snapshot, command, run};

#[test]
fn toolu_status_lists_its_planned_verbs() {
  let matches = command()
    .try_get_matches_from(["status", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu status is not ported yet (#445). Planned verbs: none (the command itself is planned)"
    )
  );
}

#[test]
fn snapshot_is_not_ported_and_names_its_issue() {
  let status: &dyn StatusSnapshot = &Snapshot;
  let roots = Roots::new(Env::default(), None);
  assert_eq!(
    status.snapshot(&roots, Path::new("/repo")),
    Err(LinkError::NotPorted { issue: 445 })
  );
}
