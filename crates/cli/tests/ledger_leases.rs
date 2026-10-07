//! Job leases across implementations (#421, AC-6): in worktrees the TypeScript
//! resources bound to one resource root, a job lease either implementation
//! holds refuses the other's check, red with empty evidence, until it is
//! released.

#[path = "helpers/interop.rs"]
mod interop;
#[path = "helpers/ledger.rs"]
mod ledger;

use std::io::{BufRead as _, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use interop::{REPO, at, bun, plan, rust, typescript};
use ledger::{Project, Res};
use serde_json::{Value, json};

/// `@toolu/core/resources` in one `bun -e`, with `RES`, `BINDING` and `ROOT` set.
fn resources(project: &Project, root: &Path, script: &str) -> Command {
  let mut command = bun(project, &["-e", script]);
  command
    .env(
      "RES",
      format!("{REPO}/packages/toolu-core/src/resources/resources.ts"),
    )
    .env(
      "BINDING",
      format!("{REPO}/packages/toolu-core/src/resources/binding.ts"),
    )
    .env("ROOT", root);
  command
}

const ACQUIRE: &str = "const r = await import(process.env.RES); \
  const lease = await r.acquireLease(process.env.ROOT, { type: 'job', key: process.env.KEY, stateDir: process.env.ROOT }); \
  console.log(lease.token); \
  if (process.env.HOLD) setInterval(() => {}, 1000); else await r.releaseLease(process.env.ROOT, lease.token);";

const BIND: &str = "const b = await import(process.env.BINDING); \
  b.bindWorktree(process.env.ROOT, process.cwd(), 'agent', process.env.ROOT);";

/// A TypeScript job lease, held until the child is killed.
fn hold(project: &Project, root: &Path) -> Res<Child> {
  let mut child = resources(project, root, ACQUIRE)
    .env("KEY", "holder")
    .env("HOLD", "1")
    .stdout(Stdio::piped())
    .spawn()?;
  let stdout = child.stdout.take().ok_or("no stdout")?;
  let mut line = String::new();
  BufReader::new(stdout).read_line(&mut line)?;
  if line.trim().is_empty() {
    return Err("the TypeScript lease was not acquired".into());
  }
  Ok(child)
}

fn wait_for(file: &Path) -> Res<()> {
  let deadline = Instant::now() + Duration::from_secs(30);
  while !file.exists() {
    if Instant::now() > deadline {
      return Err(format!("{} never appeared", file.display()).into());
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  Ok(())
}

/// `project`'s worktree bound to `root`, which has no pressure holds, with a
/// plan whose s1 passes and whose s2 waits for `<home>/release`.
fn bound(project: &Project, root: &Path) -> Res<()> {
  std::fs::create_dir_all(root)?;
  std::fs::write(root.join("policy.json"), "{\"pressure\":false}\n")?;
  let (started, release) = (project.home.join("started"), project.home.join("release"));
  let wait = format!(
    "touch '{}'; while [ ! -f '{}' ]; do sleep 0.05; done",
    started.display(),
    release.display()
  );
  plan(
    project,
    &json!([
      {"id": "s1", "title": "quick", "check": "true", "ac_refs": ["AC-1"]},
      {"id": "s2", "title": "held", "check": wait, "ac_refs": ["AC-2"]},
    ]),
  )?;
  if !resources(project, root, BIND).status()?.success() {
    return Err("bindWorktree failed".into());
  }
  project.sh("test -f .git/toolu-resource.json")?;
  Ok(())
}

/// The job leases in `root`'s state, as `(ownerPid, key)`.
fn jobs(root: &Path) -> Res<Vec<(Value, Value)>> {
  let state: Value = serde_json::from_str(&std::fs::read_to_string(root.join("state.json"))?)?;
  let leases = state
    .get("leases")
    .and_then(Value::as_array)
    .into_iter()
    .flatten();
  let job = |lease: &&Value| lease.get("type").and_then(Value::as_str) == Some("job");
  let field = |lease: &Value, key: &str| lease.get(key).cloned().unwrap_or(Value::Null);
  Ok(
    leases
      .filter(job)
      .map(|lease| (field(lease, "ownerPid"), field(lease, "key")))
      .collect(),
  )
}

/// Step `pointer`'s status and evidence after an admission refusal.
fn refusal(project: &Project, step: usize) -> Res<(Value, bool)> {
  let evidence = at(project, &format!("/steps/{step}/evidence_tail"))?;
  let empty = evidence.as_str().is_none_or(str::is_empty);
  Ok((at(project, &format!("/steps/{step}/status"))?, empty))
}

#[test]
fn a_typescript_job_lease_refuses_the_rust_check_until_released() {
  let project = Project::new().unwrap();
  let root = project.home.join("resources");
  bound(&project, &root).unwrap();
  let mut holder = hold(&project, &root).unwrap();
  let refused = rust(&project, &["run", "plan.md", "--step", "s1"]).unwrap();
  assert_eq!(refused.status.code(), Some(1));
  assert_eq!(refusal(&project, 0).unwrap(), (Value::from("red"), true));
  holder.kill().unwrap();
  holder.wait().unwrap();
  let admitted = rust(&project, &["run", "plan.md", "--step", "s1"]).unwrap();
  let stderr = String::from_utf8_lossy(&admitted.stderr);
  assert_eq!(
    at(&project, "/steps/0/status").unwrap(),
    "green",
    "{stderr}"
  );
}

#[test]
fn a_running_rust_check_holds_the_job_slot_against_typescript() {
  let (project, other) = (Project::new().unwrap(), Project::new().unwrap());
  let root = project.home.join("resources");
  bound(&project, &root).unwrap();
  bound(&other, &root).unwrap();
  let mut running = project
    .command(&project.root)
    .args(["ledger", "run", "plan.md", "--step", "s2"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();
  wait_for(&project.home.join("started")).unwrap();
  let held = jobs(&root).unwrap();
  assert_eq!(held.len(), 1, "{held:?}");
  assert_eq!(held[0].0, running.id(), "the job lease is not toolu's");

  let contended = resources(&project, &root, ACQUIRE)
    .env("KEY", "ts")
    .output()
    .unwrap();
  assert!(String::from_utf8_lossy(&contended.stderr).contains("job capacity exhausted (1)"));
  let step = typescript(&other, &["run", "plan.md", "--step", "s1"]).unwrap();
  assert_eq!(step.status.code(), Some(1));
  assert_eq!(refusal(&other, 0).unwrap(), (Value::from("red"), true));

  std::fs::write(project.home.join("release"), "").unwrap();
  running.wait().unwrap();
  assert_eq!(at(&project, "/steps/1/status").unwrap(), "green");
  assert_eq!(
    jobs(&root).unwrap(),
    Vec::new(),
    "the lease outlived the check"
  );
  let free = resources(&project, &root, ACQUIRE)
    .env("KEY", "ts")
    .output()
    .unwrap();
  assert!(
    free.status.success(),
    "{}",
    String::from_utf8_lossy(&free.stderr)
  );
}
