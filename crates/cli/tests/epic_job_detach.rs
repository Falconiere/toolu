//! `toolu epic job --detach` from the real binary: the launcher returns an
//! id at once, the copy owns the job lease while it runs, and the record
//! reaches `done` with the exit code and the failure tail.

#[path = "helpers/epic.rs"]
mod epic;

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use epic::{Home, Res, err};

fn bound(home: &Home) -> Res<std::path::PathBuf> {
  let work = home.path("work");
  std::fs::create_dir_all(&work)?;
  let init = Command::new("git")
    .args(["init", "-q"])
    .current_dir(&work)
    .status()?;
  if !init.success() {
    return Err(std::io::Error::other("git init failed").into());
  }
  let git_dir = work.join(".git");
  let binding = serde_json::json!({
    "version": 1,
    "root": home.path("resources").display().to_string(),
    "key": "detached",
    "stateDir": home.path("epic").display().to_string(),
    "worktree": std::fs::canonicalize(&work)?.display().to_string(),
  });
  std::fs::write(git_dir.join("toolu-resource.json"), format!("{binding}\n"))?;
  Ok(work)
}

fn record(home: &Home, id: &str) -> Res<Value> {
  let text = std::fs::read_to_string(home.path(&format!("resources/jobs/results/{id}.json")))?;
  Ok(serde_json::from_str(&text)?)
}

fn leases(home: &Home) -> Vec<Value> {
  let Ok(text) = std::fs::read_to_string(home.path("resources/state.json")) else {
    return Vec::new();
  };
  let Ok(state) = serde_json::from_str::<Value>(&text) else {
    return Vec::new();
  };
  state
    .get("leases")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default()
}

fn launch(home: &Home, work: &Path) -> Res<(Output, u32)> {
  let mut command = home.command(&[
    "epic",
    "job",
    "--detach",
    "--",
    "sh",
    "-c",
    "sleep 2; exit 3",
  ]);
  let child = command
    .current_dir(work)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  let pid = child.id();
  Ok((child.wait_with_output()?, pid))
}

fn detach(home: &Home, work: &Path, script: &str) -> Res<String> {
  let output = home
    .command(&["epic", "job", "--detach", "--", "sh", "-c", script])
    .current_dir(work)
    .output()?;
  if !output.status.success() {
    return Err(std::io::Error::other(format!("detach failed: {}", err(&output))).into());
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn wait(home: &Home, work: &Path, args: &[&str]) -> Res<Output> {
  Ok(home.command(args).current_dir(work).output()?)
}

#[test]
fn a_detached_job_returns_an_id_and_records_its_failure() {
  let home = Home::new().unwrap();
  let usage = home.run(&["epic", "job", "--detach"]).unwrap();
  assert_eq!(usage.status.code(), Some(64), "{}", err(&usage));
  let work = bound(&home).unwrap();
  let started = Instant::now();
  let (output, launcher_pid) = launch(&home, &work).unwrap();
  assert!(
    started.elapsed() < Duration::from_secs(1),
    "launcher took {:?}",
    started.elapsed()
  );
  assert_eq!(output.status.code(), Some(0), "{}", err(&output));
  let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
  assert!(!id.is_empty(), "no job id printed");
  let mut saw_copy_lease = false;
  let deadline = Instant::now() + Duration::from_secs(15);
  loop {
    saw_copy_lease |= leases(&home).iter().any(|lease| {
      let owner = lease.get("ownerPid").and_then(Value::as_u64).unwrap_or(0);
      lease.get("type").and_then(Value::as_str) == Some("job")
        && owner != u64::from(launcher_pid)
        && owner != 0
    });
    if let Ok(done) = record(&home, &id)
      && done["state"] == "done"
    {
      assert_eq!(done["exit_code"], 3, "{done}");
      assert!(
        done["output"]
          .as_str()
          .unwrap_or("")
          .contains("command exited 3"),
        "{done}"
      );
      break;
    }
    assert!(Instant::now() < deadline, "record never reached done");
    std::thread::sleep(Duration::from_millis(50));
  }
  assert!(saw_copy_lease, "the copy never held the job lease");
  assert!(
    leases(&home)
      .iter()
      .all(|lease| lease.get("type").and_then(Value::as_str) != Some("job")),
    "job lease remains after the copy ended"
  );
}

#[test]
fn wait_replays_failure_and_claims_without_a_resource_binding() {
  let home = Home::new().unwrap();
  let work = bound(&home).unwrap();
  let id = detach(&home, &work, "sleep 2; exit 3").unwrap();
  let deadline = Instant::now() + Duration::from_secs(10);
  while record(&home, &id).unwrap()["state"] != "running" {
    assert!(Instant::now() < deadline, "detached copy never started");
    std::thread::sleep(Duration::from_millis(20));
  }
  std::fs::remove_file(work.join(".git/toolu-resource.json")).unwrap();
  let started = Instant::now();
  let output = wait(&home, &work, &["epic", "job", "--wait", &id]).unwrap();
  assert!(started.elapsed() >= Duration::from_secs(1));
  assert_eq!(output.status.code(), Some(1), "{}", err(&output));
  assert!(
    err(&output).contains("command exited 3"),
    "{}",
    err(&output)
  );
  assert!(record(&home, &id).unwrap()["delivered_at"].is_string());
  let again = wait(&home, &work, &["epic", "job", "--wait", &id]).unwrap();
  assert_eq!(again.status.code(), Some(1));
}

#[test]
fn wait_empty_timeout_and_invalid_command() {
  let home = Home::new().unwrap();
  let work = bound(&home).unwrap();
  let started = Instant::now();
  let empty = wait(&home, &work, &["epic", "job", "--wait"]).unwrap();
  assert_eq!(empty.status.code(), Some(0), "{}", err(&empty));
  assert!(started.elapsed() < Duration::from_secs(5));
  let usage = wait(&home, &work, &["epic", "job", "--wait", "--", "true"]).unwrap();
  assert_eq!(usage.status.code(), Some(64), "{}", err(&usage));
  let id = detach(&home, &work, "sleep 3; printf done").unwrap();
  let timed = wait(
    &home,
    &work,
    &["epic", "job", "--wait", &id, "--max-seconds", "1"],
  )
  .unwrap();
  assert_eq!(timed.status.code(), Some(1), "{}", err(&timed));
  assert!(err(&timed).contains(&id), "{}", err(&timed));
  assert!(record(&home, &id).unwrap()["delivered_at"].is_null());
  let done = wait(&home, &work, &["epic", "job", "--wait", &id]).unwrap();
  assert_eq!(done.status.code(), Some(0), "{}", err(&done));
  assert!(String::from_utf8_lossy(&done.stdout).contains("done"));
}

#[test]
fn wait_without_id_claims_only_oldest_and_rejects_foreign_worktree() {
  let home = Home::new().unwrap();
  let work = bound(&home).unwrap();
  let first = detach(&home, &work, "printf first").unwrap();
  let second = detach(&home, &work, "printf second").unwrap();
  let foreign = home.path("other");
  std::fs::create_dir_all(&foreign).unwrap();
  assert!(
    Command::new("git")
      .arg("init")
      .arg("-q")
      .current_dir(&foreign)
      .status()
      .unwrap()
      .success()
  );
  let denied = wait(&home, &foreign, &["epic", "job", "--wait", &first]).unwrap();
  assert_eq!(denied.status.code(), Some(1), "{}", err(&denied));
  assert!(record(&home, &first).unwrap()["delivered_at"].is_null());
  let one = wait(&home, &work, &["epic", "job", "--wait"]).unwrap();
  assert_eq!(one.status.code(), Some(0), "{}", err(&one));
  assert!(String::from_utf8_lossy(&one.stdout).contains("first"));
  assert!(record(&home, &first).unwrap()["delivered_at"].is_string());
  assert!(record(&home, &second).unwrap()["delivered_at"].is_null());
  let two = wait(&home, &work, &["epic", "job", "--wait"]).unwrap();
  assert_eq!(two.status.code(), Some(0), "{}", err(&two));
  assert!(String::from_utf8_lossy(&two.stdout).contains("second"));
  assert!(record(&home, &second).unwrap()["delivered_at"].is_string());
}
