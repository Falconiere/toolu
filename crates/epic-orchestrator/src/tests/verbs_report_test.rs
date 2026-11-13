use super::{PHASES, job_command, report_command};

#[test]
fn report_takes_a_known_phase_and_job_takes_trailing_argv() {
  for phase in PHASES {
    report_command()
      .try_get_matches_from(["report", phase, "--status-file", "status.json"])
      .expect(phase);
  }
  assert!(
    report_command()
      .try_get_matches_from(["report", "unknown", "--status-file", "status.json"])
      .is_err()
  );
  assert!(
    report_command()
      .try_get_matches_from(["report", "ready", "--status-file", "s", "--run", "r"])
      .is_err(),
    "--run requires --expected-base"
  );
  let job = job_command()
    .try_get_matches_from(["job", "--no-wait", "--", "cargo", "--version"])
    .expect("job");
  assert!(job.get_flag("no-wait"));
  let detached = job_command()
    .try_get_matches_from(["job", "--detach", "--", "cargo", "xtask", "gate"])
    .expect("detached job");
  assert!(detached.get_flag("detach"));
  assert!(
    job_command()
      .try_get_matches_from(["job", "--detach", "--result", "x", "--", "true"])
      .is_err(),
    "--detach and --result are two ends of one run"
  );
  assert_eq!(
    job
      .get_many::<String>("argv")
      .expect("argv")
      .cloned()
      .collect::<Vec<_>>(),
    ["cargo", "--version"]
  );
}
