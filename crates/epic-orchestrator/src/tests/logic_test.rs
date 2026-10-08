use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::journal;
use crate::model::Report;
use crate::paths::Paths;
use crate::schedule::note_event;
use crate::server::{Engine, Fault, Stop, wait_body};

struct Sandbox {
  _tmp: tempfile::TempDir,
  paths: Paths,
  scripts: PathBuf,
  epic: PathBuf,
  counts: PathBuf,
}

fn sandbox(pair: bool) -> Sandbox {
  let tmp = tempfile::tempdir().expect("temp");
  let epic = tmp.path().join("epic");
  let counts = tmp.path().join("counts");
  let scripts = tmp.path().join("scripts");
  std::fs::create_dir_all(epic.join("issues")).expect("issues");
  std::fs::create_dir_all(epic.join("status")).expect("status");
  std::fs::create_dir_all(&counts).expect("counts");
  std::fs::create_dir_all(&scripts).expect("scripts");
  write_graph(&epic, pair);
  write_registry(tmp.path(), &epic);
  write_scripts(&scripts, &counts);
  git_init(&epic);
  Sandbox {
    paths: Paths::at(tmp.path()),
    scripts,
    epic,
    counts,
    _tmp: tmp,
  }
}

fn write_graph(epic: &Path, pair: bool) {
  let issues = if pair {
    r#"[{"key":"a","open_blockers":[]},{"key":"b","open_blockers":["a"]}]"#
  } else {
    r#"[{"key":"a","open_blockers":[]}]"#
  };
  let body = format!(r#"{{"epic":"one","issues":{issues}}}"#);
  std::fs::write(epic.join("graph.json"), body).expect("graph");
}

fn write_registry(root: &Path, epic: &Path) {
  let body = format!(
    r#"{{"version":1,"epics":[{{"key":"one","state_dir":"{}"}}]}}"#,
    epic.display()
  );
  std::fs::write(root.join("registry.json"), body).expect("registry");
}

fn write_scripts(scripts: &Path, counts: &Path) {
  let counts = counts.display().to_string();
  write_exe(
    scripts,
    "launch",
    "#!/bin/sh\necho '{\"outcome\":\"applied\"}'\n",
  );
  write_exe(
    scripts,
    "merge",
    &format!(
      "#!/bin/sh\necho x >> '{counts}/merge.count'\necho merged > '{counts}/merge.marker'\necho '{{\"outcome\":\"applied\"}}'\n"
    ),
  );
  write_exe(
    scripts,
    "cleanup",
    &format!("#!/bin/sh\necho x >> '{counts}/cleanup.count'\necho '{{\"outcome\":\"applied\"}}'\n"),
  );
  write_exe(
    scripts,
    "reconcile",
    &format!(
      "#!/bin/sh\nif [ -f '{counts}/merge.marker' ]; then echo '{{\"state\":\"merged\"}}'; else echo '{{\"state\":\"open\"}}'; fi\n"
    ),
  );
}

fn write_exe(dir: &Path, name: &str, body: &str) {
  let path = dir.join(name);
  std::fs::write(&path, body).expect("script");
  let mut perms = std::fs::metadata(&path).expect("meta").permissions();
  std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
  std::fs::set_permissions(&path, perms).expect("chmod");
}

fn git_init(dir: &Path) {
  let status = std::process::Command::new("git")
    .args(["init", "-q"])
    .current_dir(dir)
    .status()
    .expect("git init");
  assert!(status.success());
}

fn open(sandbox: &Sandbox, fault: Fault) -> Engine {
  Engine::open(sandbox.paths.clone(), Some(sandbox.scripts.clone()), fault).expect("open")
}

fn lines(path: &Path) -> usize {
  match std::fs::read_to_string(path) {
    Ok(text) => text.lines().count(),
    Err(_) => 0,
  }
}

fn ready_report(engine: &Engine, key: &str) -> Report {
  let issue = engine.world.issues.get(key).expect("issue");
  Report {
    key: key.to_owned(),
    epic: issue.epic.clone(),
    state_dir: issue.state_dir.clone(),
    phase: "ready".to_owned(),
    pr: Some(7),
    note: String::new(),
  }
}

fn launch_ready(engine: &mut Engine) {
  assert_eq!(engine.pump().expect("launch"), Stop::Idle);
  let report = ready_report(engine, "a");
  engine.report(&report).expect("ready");
}

fn settle(engine: &mut Engine) {
  for _ in 0..6 {
    assert_eq!(engine.pump().expect("pump"), Stop::Idle);
    let keys = running_unphased(engine);
    if keys.is_empty() {
      return;
    }
    report_each(engine, &keys);
  }
  panic!("engine did not settle");
}

fn running_unphased(engine: &Engine) -> Vec<String> {
  engine
    .world
    .issues
    .values()
    .filter(|issue| issue.stage == "running" && issue.phase.is_empty())
    .map(|issue| issue.key.clone())
    .collect()
}

fn report_each(engine: &mut Engine, keys: &[String]) {
  for key in keys {
    let report = ready_report(engine, key);
    engine.report(&report).expect("ready");
  }
}

fn assert_no_wait(sandbox: &Sandbox) {
  let records = journal::tail(&sandbox.paths.journal_dir(), SystemTime::now()).expect("journal");
  assert!(records.iter().all(|record| record.name != "wait"));
}

fn assert_wait(engine: &mut Engine, epic: &Path) {
  note_event(&mut engine.world, "a", "host-limited");
  engine.flush().expect("flush");
  assert_eq!(wait_body(&mut engine.world, 0)["state"], "waiting");
  engine
    .report(&Report {
      key: "c".to_owned(),
      epic: "one".to_owned(),
      state_dir: epic.display().to_string(),
      phase: "needs-human".to_owned(),
      pr: None,
      note: "look".to_owned(),
    })
    .expect("needs-human");
  assert_eq!(wait_body(&mut engine.world, 0)["kind"], "needs-human");
}

#[test]
fn scripted_epic() {
  let sandbox = sandbox(true);
  let mut engine = open(&sandbox, Fault::None);
  settle(&mut engine);
  assert_eq!(engine.world.issues.get("a").expect("a").stage, "merged");
  assert_eq!(engine.world.issues.get("b").expect("b").stage, "merged");
  assert_eq!(lines(&sandbox.counts.join("merge.count")), 2);
  assert_eq!(lines(&sandbox.counts.join("cleanup.count")), 2);
  assert_no_wait(&sandbox);
  assert_wait(&mut engine, &sandbox.epic);
}

#[test]
fn merge_fault() {
  fault_case(Fault::BeforeMerge);
  fault_case(Fault::AfterMerge);
  fault_case(Fault::BeforeMergeRecord);
}

fn fault_case(fault: Fault) {
  let sandbox = sandbox(false);
  let mut engine = open(&sandbox, fault);
  launch_ready(&mut engine);
  assert_eq!(engine.pump().expect("fault"), Stop::Fault);
  drop(engine);
  let mut engine = open(&sandbox, Fault::None);
  settle(&mut engine);
  assert_eq!(lines(&sandbox.counts.join("merge.count")), 1, "{fault:?}");
  assert_eq!(lines(&sandbox.counts.join("cleanup.count")), 1, "{fault:?}");
  assert_eq!(engine.world.issues.get("a").expect("a").stage, "merged");
}

#[test]
fn pause_survives() {
  let sandbox = sandbox(false);
  let mut engine = open(&sandbox, Fault::None);
  launch_ready(&mut engine);
  engine.pause(None).expect("pause");
  assert_eq!(engine.pump().expect("paused"), Stop::Idle);
  assert_eq!(lines(&sandbox.counts.join("merge.count")), 0);
  drop(engine);
  let mut engine = open(&sandbox, Fault::None);
  assert_eq!(engine.pump().expect("still paused"), Stop::Idle);
  assert_eq!(lines(&sandbox.counts.join("merge.count")), 0);
  engine.resume(None).expect("resume");
  settle(&mut engine);
  assert_eq!(lines(&sandbox.counts.join("merge.count")), 1);
}
