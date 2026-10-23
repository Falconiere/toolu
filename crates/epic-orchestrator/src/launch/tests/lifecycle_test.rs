use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{
  Expected, Outcome, acknowledged_outcome, acknowledged_status, await_named_agent, inspect_agent,
  shutdown_agent,
};
use crate::herdr::Call;
use crate::herdr_state::Scripted;
use crate::hosts::HostKind;
use crate::launch_rig::Rig;

/// A worktree with a started agent of `kind`: its path and pane.
fn started(rig: &Rig, kind: &str) -> (PathBuf, String) {
  let worktree = rig.tmp.path().join("wt");
  std::fs::create_dir_all(&worktree).expect("worktree");
  let workspace = rig.herdr.add_repo(&worktree);
  let pane = format!("{workspace}:p1");
  let mut ctx = rig.ctx();
  let start = Call::Start {
    name: "k-1".into(),
    kind: kind.into(),
    pane_id: pane.clone(),
    args: Vec::new(),
    timeout_ms: None,
  };
  ctx.herdr.call(&start).expect("start");
  (std::fs::canonicalize(&worktree).expect("real"), pane)
}

fn expected<'a>(kind: HostKind, pane: &'a str, cwd: &'a Path) -> Expected<'a> {
  Expected {
    kind,
    pane,
    cwd: cwd.to_str().expect("utf8"),
  }
}

#[test]
fn a_limit_in_the_tail_wins_and_blocked_is_kept() {
  assert_eq!(
    acknowledged_status("working", "you hit your usage limit", Outcome::Started),
    Outcome::ProviderLimited
  );
  assert_eq!(
    acknowledged_status("blocked", "Allow this command?", Outcome::Started),
    Outcome::Blocked
  );
  assert_eq!(
    acknowledged_status("working", "", Outcome::Resumed),
    Outcome::Resumed
  );
  assert_eq!(Outcome::CleanupIncomplete.name(), "cleanup-incomplete");
}

#[test]
fn a_live_agent_is_verified_by_host_pane_and_worktree() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "claude");
  let mut ctx = rig.ctx();
  let live = inspect_agent(&mut ctx.herdr, "k-1")
    .expect("inspect")
    .expect("live");
  assert_eq!(
    (live.kind, live.pane.as_str()),
    (HostKind::Claude, pane.as_str())
  );
  assert_eq!(inspect_agent(&mut ctx.herdr, "nobody"), Ok(None));
  let wrong = expected(HostKind::Codex, &pane, &worktree);
  let mismatch =
    acknowledged_outcome(&mut ctx.herdr, "k-1", &wrong, Outcome::Started).expect_err("identity");
  assert_eq!(
    mismatch.message,
    "live agent k-1 identity does not match recorded host/pane/worktree"
  );
  rig.herdr.state().agents[0].status = "blocked".into();
  rig.herdr.state().tail = "Error: rate limited, try again in 5 minutes".into();
  let right = expected(HostKind::Claude, &pane, &worktree);
  assert_eq!(
    acknowledged_outcome(&mut ctx.herdr, "k-1", &right, Outcome::Started),
    Ok(Outcome::ProviderLimited)
  );
  rig.herdr.script(
    "agent.list",
    Scripted::Raw(r#"{"result":{"agents":[{"name":"k-1"}]}}"#.into()),
  );
  let partial = inspect_agent(&mut ctx.herdr, "k-1").expect_err("partial");
  assert_eq!(partial.message, "cannot verify live identity of k-1");
  rig
    .herdr
    .script("agent.list", Scripted::Raw(r#"{"result":{}}"#.into()));
  assert_eq!(
    inspect_agent(&mut ctx.herdr, "k-1")
      .expect_err("omitted")
      .message,
    "herdr agent list omitted agents"
  );
}

#[test]
fn a_started_agent_is_addressable_before_its_first_prompt() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "claude");
  let mut ctx = rig.ctx();
  let right = expected(HostKind::Claude, &pane, &worktree);
  // The test server reports a fresh agent as working: listed, but not
  // promptable yet.
  let busy = await_named_agent(&mut ctx.herdr, "k-1", &right, Duration::ZERO).expect_err("busy");
  assert!(busy.uncertain);
  assert_eq!(
    busy.message,
    "agent k-1 is not addressable after start; inspect the owned pane before retry"
  );
  rig.herdr.state().agents[0].status = "idle".into();
  await_named_agent(&mut ctx.herdr, "k-1", &right, Duration::from_secs(5)).expect("named");
  let wrong = expected(HostKind::Codex, &pane, &worktree);
  let mismatch =
    await_named_agent(&mut ctx.herdr, "k-1", &wrong, Duration::ZERO).expect_err("identity");
  assert!(!mismatch.uncertain);
  assert_eq!(
    mismatch.message,
    "live agent k-1 identity does not match recorded host/pane/worktree"
  );
  let absent =
    await_named_agent(&mut ctx.herdr, "nobody", &right, Duration::ZERO).expect_err("absent");
  assert!(absent.uncertain);
  assert_eq!(
    absent.message,
    "agent nobody is not addressable after start; inspect the owned pane before retry"
  );
  rig.herdr.script(
    "agent.list",
    Scripted::Raw(r#"{"result":{"agents":[{"name":"nobody"}]}}"#.into()),
  );
  let pending =
    await_named_agent(&mut ctx.herdr, "nobody", &right, Duration::ZERO).expect_err("pending");
  assert!(pending.uncertain);
  assert_eq!(pending.message, absent.message);
}

#[test]
fn shutdown_exits_the_agent_and_codex_also_stops_its_jobs() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "codex");
  let mut ctx = rig.ctx();
  assert_eq!(
    shutdown_agent(
      &mut ctx.herdr,
      "k-1",
      &expected(HostKind::Codex, &pane, &worktree)
    ),
    Ok(Outcome::Exited)
  );
  let methods: Vec<String> = rig
    .herdr
    .calls()
    .into_iter()
    .map(|(method, _)| method)
    .collect();
  let tail: Vec<&str> = methods
    .iter()
    .map(String::as_str)
    .filter(|m| *m != "agent.list")
    .collect();
  assert_eq!(
    tail,
    [
      "agent.start",
      "pane.process_info",
      "agent.send_keys",
      "agent.wait",
      "agent.prompt",
      "agent.wait",
      "agent.prompt"
    ]
  );
  assert_eq!(
    shutdown_agent(
      &mut ctx.herdr,
      "k-1",
      &expected(HostKind::Codex, &pane, &worktree)
    ),
    Ok(Outcome::Exited)
  );
}

#[test]
fn workload_left_in_the_worktree_is_cleanup_incomplete() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "claude");
  // Its own group, as a worker's job has: in the test runner's group, any
  // exiting process of a parallel test would make ownership unknowable.
  let mut leftover = std::os::unix::process::CommandExt::process_group(
    std::process::Command::new("sleep")
      .arg("30")
      .current_dir(&worktree),
    0,
  )
  .spawn()
  .expect("leftover");
  let mut ctx = rig.ctx();
  let outcome = shutdown_agent(
    &mut ctx.herdr,
    "k-1",
    &expected(HostKind::Claude, &pane, &worktree),
  );
  let _killed = leftover.kill();
  let _reaped = leftover.wait();
  assert_eq!(outcome, Ok(Outcome::CleanupIncomplete));
  let keys = rig
    .herdr
    .calls()
    .into_iter()
    .filter(|(method, _)| method == "agent.send_keys")
    .map(|(_, params)| params["keys"].clone())
    .collect::<Vec<_>>();
  assert_eq!(keys, [serde_json::json!(["esc"])]);
  rig.herdr.script(
    "pane.process_info",
    Scripted::Raw(r#"{"result":{"process_info":{}}}"#.into()),
  );
  let blind = shutdown_agent(
    &mut ctx.herdr,
    "k-1",
    &expected(HostKind::Claude, &pane, &worktree),
  )
  .expect_err("blind");
  assert_eq!(blind.message, "pane shell ownership unavailable");
}
