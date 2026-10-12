use std::path::{Path, PathBuf};

use super::{
  Expected, Outcome, acknowledged_outcome, acknowledged_status, inspect_agent, shutdown_agent,
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
  let (worktree, pane) = started(&rig, "opencode");
  let mut leftover = std::process::Command::new("sleep")
    .arg("30")
    .current_dir(&worktree)
    .spawn()
    .expect("leftover");
  let mut ctx = rig.ctx();
  let outcome = shutdown_agent(
    &mut ctx.herdr,
    "k-1",
    &expected(HostKind::Opencode, &pane, &worktree),
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
  assert_eq!(keys, [serde_json::json!(["ctrl+c"])]);
  rig.herdr.script(
    "pane.process_info",
    Scripted::Raw(r#"{"result":{"process_info":{}}}"#.into()),
  );
  let blind = shutdown_agent(
    &mut ctx.herdr,
    "k-1",
    &expected(HostKind::Opencode, &pane, &worktree),
  )
  .expect_err("blind");
  assert_eq!(blind.message, "pane shell ownership unavailable");
}
