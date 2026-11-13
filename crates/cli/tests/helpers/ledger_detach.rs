//! Detached ledger runs against the real binary, Git, result files, and job lease.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::ledger::{Project, Res};

fn plan(check: &str) -> String {
  let steps = json!([{"id":"s1","title":"one","check":check,"ac_refs":["AC-1"]}]);
  format!("# Plan\n\n## Steps (machine-readable)\n\n```json\n{steps}\n```\n")
}

fn run(project: &Project, args: &[&str]) -> Res<Output> {
  Ok(project.command(&project.root).args(args).output()?)
}

fn id(output: &Output) -> String {
  assert_eq!(
    output.status.code(),
    Some(0),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
  assert!(!id.is_empty(), "no job id: {output:?}");
  id
}

fn result(root: &Path, id: &str) -> Option<Value> {
  let file = root.join(format!("jobs/results/{id}.json"));
  let bytes = std::fs::read(file).ok()?;
  serde_json::from_slice(&bytes).ok()
}

fn wait_done(root: &Path, id: &str) -> Value {
  let deadline = Instant::now() + Duration::from_secs(15);
  loop {
    if let Some(record) = result(root, id)
      && record["state"] == "done"
    {
      return record;
    }
    assert!(Instant::now() < deadline, "job {id} never finished");
    std::thread::sleep(Duration::from_millis(50));
  }
}

fn wait_file(path: &Path) {
  let deadline = Instant::now() + Duration::from_secs(15);
  while !path.exists() {
    assert!(
      Instant::now() < deadline,
      "{} was not created",
      path.display()
    );
    std::thread::sleep(Duration::from_millis(50));
  }
}

fn resource_root(project: &Project) -> PathBuf {
  project.home.join(".local/state/toolu/resources")
}

#[test]
fn detached_passing_plan_returns_before_check_and_records_green() {
  let project = Project::new().unwrap();
  let resource = project.home.join("bound-resources");
  let binding = json!({
    "version": 1,
    "root": resource.display().to_string(),
    "key": "detached-ledger",
    "stateDir": project.home.display().to_string(),
    "worktree": project.root.display().to_string(),
  });
  std::fs::write(
    project.root.join(".git/toolu-resource.json"),
    binding.to_string(),
  )
  .unwrap();
  let marker = project.home.join("started");
  project
    .write(
      "plan.md",
      &plan(&format!("touch '{}' && sleep 2", marker.display())),
    )
    .unwrap();
  let start = Instant::now();
  let output = run(&project, &["ledger", "run", "plan.md", "--detach"]).unwrap();
  let id = id(&output);
  let launched = result(&resource, &id).unwrap();
  assert_ne!(
    launched["state"],
    "done",
    "launcher took {:?} and waited for the check: {launched}",
    start.elapsed()
  );
  wait_file(&marker);
  let state: Value =
    serde_json::from_slice(&std::fs::read(resource.join("state.json")).unwrap()).unwrap();
  assert!(
    state["leases"]
      .as_array()
      .unwrap()
      .iter()
      .any(|lease| lease["type"] == "job"),
    "no child job lease: {state}"
  );
  let done = wait_done(&resource, &id);
  assert_eq!(done["exit_code"], 0, "{done}");
  assert_eq!(done["stale"], false, "{done}");
  assert_eq!(done["key"], "detached-ledger");
  assert!(done["output"].as_str().unwrap().contains("1/1 fresh-green"));
  let ledger: Value = serde_json::from_slice(&std::fs::read(project.ledger()).unwrap()).unwrap();
  assert_eq!(ledger["steps"][0]["status"], "green");
}

#[test]
fn relative_plan_path_uses_the_launch_directory() {
  let project = Project::new().unwrap();
  project.write("sub/plan.md", &plan("true")).unwrap();
  let output = project
    .command(&project.root.join("sub"))
    .args(["ledger", "run", "plan.md", "--detach"])
    .output()
    .unwrap();
  let id = id(&output);
  let done = wait_done(&resource_root(&project), &id);
  assert_eq!(done["exit_code"], 0, "{done}");
  let ledger: Value = serde_json::from_slice(&std::fs::read(project.ledger()).unwrap()).unwrap();
  assert_eq!(ledger["steps"][0]["status"], "green");
}

fn stale_run(step_only: bool) {
  let project = Project::new().unwrap();
  project.write("plan.md", &plan("true")).unwrap();
  let first = run(&project, &["ledger", "run", "plan.md"]).unwrap();
  assert_eq!(first.status.code(), Some(0), "{first:?}");
  let prior = std::fs::read(project.ledger()).unwrap();
  let marker = project.home.join("started");
  project
    .write(
      "plan.md",
      &plan(&format!("touch '{}' && sleep 2", marker.display())),
    )
    .unwrap();
  let args = if step_only {
    &["ledger", "run", "plan.md", "--step", "s1", "--detach"][..]
  } else {
    &["ledger", "run", "plan.md", "--force", "--detach"][..]
  };
  let output = run(&project, args).unwrap();
  let id = id(&output);
  wait_file(&marker);
  project.write("base.txt", "changed\n").unwrap();
  let done = wait_done(&resource_root(&project), &id);
  assert_eq!(done["state"], "done", "{done}");
  assert!(done["exit_code"].as_i64().unwrap() != 0, "{done}");
  assert_eq!(done["stale"], true, "{done}");
  assert_eq!(done["key"], "");
  assert!(
    done["output"]
      .as_str()
      .unwrap()
      .contains("tree changed during the run")
  );
  assert_eq!(std::fs::read(project.ledger()).unwrap(), prior);
}

#[test]
fn tracked_edit_during_detached_run_leaves_ledger_unchanged() {
  stale_run(false);
}

#[test]
fn tracked_edit_during_detached_step_skips_the_running_prewrite() {
  stale_run(true);
}
