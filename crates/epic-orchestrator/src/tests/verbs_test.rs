use super::{command, run};
use crate::paths::Paths;
use crate::query::install_unit;
use toolu_runtime::cli::Ctx;

#[test]
fn planned_lists_only_the_later_verbs() {
  let matches = command()
    .try_get_matches_from(["epic", "planned"])
    .expect("planned");
  let outcome = run(&matches, &Ctx::default(), &());
  assert_eq!(
    outcome.stdout.as_deref(),
    Some(
      "toolu epic is not ported yet (#435, #448). Planned verbs: graph, route, launch, finish, close, release, jira, probe, gate, queue"
    )
  );
  command()
    .try_get_matches_from(["epic", "job", "--", "true"])
    .expect("job");
  command()
    .try_get_matches_from(["epic", "wait", "--max-seconds", "0"])
    .expect("wait");
}

#[test]
fn service_install_writes_a_unit_and_does_not_start_it() {
  let tmp = tempfile::tempdir().expect("temp");
  let home = tmp.path().join("home");
  let paths = Paths::at(tmp.path().join("resources"));
  install_unit(&paths, &home, std::path::Path::new("/usr/bin/toolu")).expect("unit");
  let body = std::fs::read_to_string(paths.service()).expect("service");
  assert!(body.contains("ExecStart=/usr/bin/toolu epic engine"));
  assert!(body.contains("WantedBy=default.target"));
  let user =
    std::fs::read_to_string(home.join(".config/systemd/user/toolu-epic.service")).expect("user");
  assert_eq!(user, body);
}
