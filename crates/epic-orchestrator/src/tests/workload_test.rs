use std::collections::BTreeSet;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::{ProcessRecord, owned_workload, select_owned, verify_owned_exit};

fn live_platform() -> bool {
  matches!(std::env::consts::OS, "linux" | "macos")
}

fn spawn(cwd: &Path, script: &str) -> Child {
  Command::new("sh")
    .args(["-c", script])
    .current_dir(cwd)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .process_group(0)
    .spawn()
    .expect("spawn")
}

fn kill_group(child: &mut Child) {
  use toolu_runtime::process::group::{Signal, signal};
  let _killed = signal(child.id(), Signal::SIGKILL);
  let _reaped = child.wait();
}

fn wait_for(path: &Path) -> u32 {
  for _ in 0..200 {
    if let Some(pid) = std::fs::read_to_string(path)
      .ok()
      .and_then(|text| text.trim().parse().ok())
    {
      return pid;
    }
    std::thread::sleep(Duration::from_millis(20));
  }
  panic!("missing process fixture {}", path.display());
}

fn record(
  pid: u32,
  parent: u32,
  group: u32,
  cwd: Option<&str>,
  unavailable: bool,
) -> ProcessRecord {
  ProcessRecord {
    pid,
    parent,
    group,
    started: format!("s{pid}"),
    cwd: cwd.map(str::to_owned),
    cwd_unavailable: unavailable,
  }
}

#[test]
fn worktree_processes_and_descendants_that_moved_are_owned() {
  if !live_platform() {
    return;
  }
  let tmp = tempfile::tempdir().expect("temp");
  let root = tmp.path().join("wt");
  let outside = tmp.path().join("outside");
  std::fs::create_dir_all(&root).expect("root");
  std::fs::create_dir_all(&outside).expect("outside");
  let pid_file = tmp.path().join("grandchild.pid");
  let script = format!(
    "sh -c \"cd '{}' && echo \\$\\$ > '{}' && exec sleep 30\" & wait",
    outside.display(),
    pid_file.display()
  );
  let mut child = spawn(&root, &script);
  let grandchild = wait_for(&pid_file);
  let records = owned_workload(&root, None).expect("inventory");
  let pids: Vec<u32> = records.iter().map(|owned| owned.pid).collect();
  assert!(
    pids.contains(&child.id()) && pids.contains(&grandchild),
    "{records:?}"
  );
  assert!(records.iter().all(|owned| owned.group == child.id()));
  assert_eq!(verify_owned_exit(&records), Ok(false));
  kill_group(&mut child);
  let mut gone = false;
  for _ in 0..200 {
    gone = verify_owned_exit(&records).expect("verify");
    if gone {
      break;
    }
    std::thread::sleep(Duration::from_millis(25));
  }
  assert!(gone);
}

#[test]
fn the_caller_and_a_supplied_shell_are_excluded() {
  if !live_platform() {
    return;
  }
  let tmp = tempfile::tempdir().expect("temp");
  let mut shell = spawn(tmp.path(), "exec sleep 30");
  std::thread::sleep(Duration::from_millis(100));
  let records = owned_workload(tmp.path(), Some(shell.id())).expect("inventory");
  kill_group(&mut shell);
  assert!(
    records
      .iter()
      .all(|owned| owned.pid != shell.id() && owned.pid != std::process::id())
  );
}

#[test]
fn an_idle_worktree_owns_nothing_and_a_missing_one_is_refused() {
  let tmp = tempfile::tempdir().expect("temp");
  if live_platform() {
    assert_eq!(owned_workload(tmp.path(), None), Ok(Vec::new()));
  }
  assert_eq!(verify_owned_exit(&[]), Ok(true));
  let missing = owned_workload(&tmp.path().join("gone"), None).expect_err("missing");
  assert!(missing.starts_with("cannot resolve worktree"), "{missing}");
}

#[test]
fn descendants_follow_their_parent_but_not_an_excluded_one() {
  let inventory = [
    record(10, 1, 10, Some("/w"), false),
    record(11, 10, 10, Some("/elsewhere"), false),
    record(12, 11, 10, None, true),
    record(20, 1, 20, Some("/w/sub"), false),
    record(21, 20, 20, Some("/w"), false),
    record(22, 21, 20, Some("/tmp"), false),
    record(30, 1, 30, Some("/w-sibling"), false),
    record(40, 1, 40, None, true),
  ];
  let owned = select_owned(&inventory, "/w", &BTreeSet::from([21])).expect("owned");
  let pids: Vec<u32> = owned.iter().map(|owned| owned.pid).collect();
  assert_eq!(pids, [10, 11, 12, 20]);
  assert_eq!(owned[0].started, "s10");
}

#[test]
fn an_unreadable_cwd_in_an_owned_group_makes_ownership_uncertain() {
  let inventory = [
    record(50, 1, 50, Some("/tmp"), false),
    record(51, 50, 50, Some("/w"), false),
    record(52, 50, 50, None, true),
  ];
  assert_eq!(
    select_owned(&inventory, "/w", &BTreeSet::new()),
    Err("cannot determine worktree ownership of process 52".to_owned())
  );
}
