//! Managed jobs over real isolated resource homes (`resources.test.ts`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use toolu_runtime::process::Spec;

use super::run_managed_job;
use crate::ledger::jq::is_str;
use crate::resources::binding::ResourceBinding;
use crate::resources::leases::{LeaseKind, LeaseRequest, acquire_lease, release_lease};
use crate::resources::store::{leases, read_resource_state};

fn binding(root: &Path, worktree: &Path) -> ResourceBinding {
  std::fs::create_dir_all(root).unwrap();
  std::fs::write(root.join("policy.json"), r#"{"maxAgents":1,"maxJobs":1}"#).unwrap();
  ResourceBinding {
    root: root.to_path_buf(),
    key: "issue-421".to_owned(),
    state_dir: root.join("epic").display().to_string(),
    worktree: worktree.to_path_buf(),
  }
}

fn argv(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

fn spec() -> Spec {
  Spec::new(Vec::<String>::new())
}

fn active_jobs(root: &Path) -> usize {
  let state = read_resource_state(root).unwrap();
  let null = toolu_runtime::json::ordered::Ordered::Null;
  leases(&state)
    .iter()
    .filter(|lease| is_str(lease.get("type").unwrap_or(&null), "job"))
    .count()
}

#[test]
fn a_job_sees_its_running_lease_and_releases_it_after() {
  let dir = tempfile::tempdir().unwrap();
  let root: PathBuf = dir.path().join("resources");
  let owned = binding(&root, dir.path());
  let state = root.join("state.json").display().to_string();
  let output = run_managed_job(&argv(&["cat", &state]), &owned, &spec()).unwrap();
  assert_eq!(output.exit_code, 0, "{}", output.stderr);
  assert!(
    output.stdout.contains("\"stage\": \"running\""),
    "{}",
    output.stdout
  );
  assert!(
    output.stdout.contains("\"groupPid\": "),
    "{}",
    output.stdout
  );
  assert!(
    output.stdout.contains("\"key\": \"issue-421:"),
    "{}",
    output.stdout
  );
  assert_eq!(active_jobs(&root), 0);
}

#[test]
fn binary_stdin_passes_through_byte_exact() {
  let dir = tempfile::tempdir().unwrap();
  let owned = binding(&dir.path().join("resources"), dir.path());
  let mut input = spec();
  input.stdin = vec![0x00, 0xff, 0xfe, 0x80, 0x0a, 0xc3];
  let output = run_managed_job(&argv(&["od", "-An", "-tx1"]), &owned, &input).unwrap();
  let bytes: Vec<&str> = output.stdout.split_whitespace().collect();
  assert_eq!(bytes, ["00", "ff", "fe", "80", "0a", "c3"]);
}

#[test]
fn capacity_is_held_until_redirected_descendants_exit() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("resources");
  let owned = binding(&root, dir.path());
  let started = Instant::now();
  let script = argv(&["sh", "-c", "sleep 0.4 >/dev/null 2>&1 &"]);
  let output = run_managed_job(&script, &owned, &spec()).unwrap();
  assert_eq!(output.exit_code, 0);
  assert!(started.elapsed() > Duration::from_millis(300));
  assert_eq!(active_jobs(&root), 0);
}

#[test]
fn a_refused_admission_spawns_nothing() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("resources");
  let owned = binding(&root, dir.path());
  let occupied = LeaseRequest {
    kind: LeaseKind::Job,
    key: "occupied".to_owned(),
    state_dir: "other".to_owned(),
    host: None,
    host_cap: None,
    epic_cap: None,
    worktree: None,
  };
  let lease = acquire_lease(&root, &occupied, SystemTime::now()).unwrap();
  let marker = dir.path().join("started");
  let script = format!("echo started > '{}'", marker.display());
  let refused = run_managed_job(&argv(&["sh", "-c", &script]), &owned, &spec());
  assert_eq!(
    refused.map(|out| out.exit_code),
    Err("job capacity exhausted (1)".to_owned())
  );
  assert!(!marker.exists());
  let token = match lease.get("token") {
    Some(toolu_runtime::json::ordered::Ordered::String(token)) => token.clone(),
    _ => panic!("no token"),
  };
  release_lease(&root, &token).unwrap();
}

#[test]
fn a_timed_out_job_reports_it_and_frees_its_lease() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("resources");
  let owned = binding(&root, dir.path());
  let mut bounded = spec();
  bounded.timeout = Duration::from_millis(300);
  let output = run_managed_job(&argv(&["sleep", "30"]), &owned, &bounded).unwrap();
  assert!(output.timed_out);
  assert_eq!(active_jobs(&root), 0);
}
