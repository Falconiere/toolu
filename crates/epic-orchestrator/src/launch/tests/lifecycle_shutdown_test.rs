//! Shutdown: the exit request per host, a degrading listing, and the grace
//! a departing agent's worktree processes get.

use std::time::Duration;

use super::tests::{expected, started};
use super::{Outcome, shutdown_agent};
use crate::herdr_state::Scripted;
use crate::hosts::HostKind;
use crate::launch_rig::Rig;

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
fn shutdown_tolerates_a_degrading_entry_while_the_agent_exits() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "claude");
  let full = format!(
    "{{\"result\":{{\"agents\":[{{\"name\":\"k-1\",\"agent\":\"claude\",\"pane_id\":\"{pane}\",\"cwd\":\"{}\",\"agent_status\":\"idle\"}}]}}}}",
    worktree.display()
  );
  rig.herdr.script("agent.list", Scripted::Raw(full));
  rig.herdr.script(
    "agent.list",
    Scripted::Raw(r#"{"result":{"agents":[{"name":"k-1"}]}}"#.into()),
  );
  let mut ctx = rig.ctx();
  assert_eq!(
    shutdown_agent(
      &mut ctx.herdr,
      "k-1",
      &expected(HostKind::Claude, &pane, &worktree)
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

#[test]
fn shutdown_confirms_muse_exit_even_when_it_already_left() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "muse");
  let mut ctx = rig.ctx();
  assert_eq!(
    shutdown_agent(
      &mut ctx.herdr,
      "k-1",
      &expected(HostKind::Muse, &pane, &worktree)
    ),
    Ok(Outcome::Exited)
  );
  let input: Vec<serde_json::Value> = rig
    .herdr
    .calls()
    .into_iter()
    .filter(|(method, _)| method == "agent.send_keys" || method == "agent.prompt")
    .map(|(_, params)| {
      params
        .get("keys")
        .or(params.get("text"))
        .cloned()
        .unwrap_or_default()
    })
    .collect();
  assert_eq!(
    input,
    [
      serde_json::json!(["esc"]),
      serde_json::json!("/exit"),
      serde_json::json!(["enter"])
    ]
  );
}

#[test]
fn already_gone_agent_on_prompt_counts_as_exited_once_unlisted() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "claude");
  // The agent's process dies under herdr: its listing degrades, then drops it.
  {
    let mut state = rig.herdr.state();
    let agent = state
      .agents
      .iter_mut()
      .find(|agent| agent.name == "k-1")
      .expect("agent");
    crate::herdr_state::stop(&mut agent.process);
  }
  let full = format!(
    "{{\"result\":{{\"agents\":[{{\"name\":\"k-1\",\"agent\":\"claude\",\"pane_id\":\"{pane}\",\"cwd\":\"{}\",\"agent_status\":\"idle\"}}]}}}}",
    worktree.display()
  );
  rig.herdr.script("agent.list", Scripted::Raw(full));
  rig.herdr.script(
    "agent.list",
    Scripted::Raw(r#"{"result":{"agents":[]}}"#.into()),
  );
  rig.herdr.script(
    "agent.prompt",
    Scripted::Error {
      code: "agent_not_found".into(),
      message: "scripted".into(),
    },
  );
  let mut ctx = rig.ctx();
  assert_eq!(
    shutdown_agent(
      &mut ctx.herdr,
      "k-1",
      &expected(HostKind::Claude, &pane, &worktree)
    ),
    Ok(Outcome::Exited)
  );
}

#[test]
fn already_gone_agent_on_exit_prompt_is_tolerated_while_listed() {
  let mut rig = Rig::new();
  rig.env = rig.env.clone().with("EPIC_AGENT_EXIT_WAIT_S", "1");
  let (worktree, pane) = started(&rig, "claude");
  rig.herdr.script(
    "agent.prompt",
    Scripted::Error {
      code: "agent_not_found".into(),
      message: "scripted".into(),
    },
  );
  let mut ctx = rig.ctx();
  // Still listed, so the shutdown is incomplete — but the missing agent
  // itself is not the failure.
  assert_eq!(
    shutdown_agent(
      &mut ctx.herdr,
      "k-1",
      &expected(HostKind::Claude, &pane, &worktree)
    ),
    Ok(Outcome::CleanupIncomplete)
  );
}

#[test]
fn already_gone_agent_during_codex_settle_counts_as_exited() {
  let rig = Rig::new();
  let (worktree, pane) = started(&rig, "codex");
  rig.herdr.script(
    "agent.wait",
    Scripted::Error {
      code: "agent_not_found".into(),
      message: "scripted".into(),
    },
  );
  let mut ctx = rig.ctx();
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
fn worktree_processes_that_exit_within_the_grace_complete_the_shutdown() {
  let mut rig = Rig::new();
  rig.env = rig.env.clone().with("EPIC_AGENT_EXIT_GRACE_S", "10");
  let (worktree, pane) = started(&rig, "claude");
  // Its own group, as a worker's job has; a thread reaps it, so it does not
  // linger as a zombie the inventory still lists.
  let mut late = std::os::unix::process::CommandExt::process_group(
    std::process::Command::new("sleep")
      .arg("3")
      .current_dir(&worktree),
    0,
  )
  .spawn()
  .expect("late");
  let reaper = std::thread::spawn(move || late.wait());
  let begun = std::time::Instant::now();
  let mut ctx = rig.ctx();
  let outcome = shutdown_agent(
    &mut ctx.herdr,
    "k-1",
    &expected(HostKind::Claude, &pane, &worktree),
  );
  let _reaped = reaper.join();
  assert_eq!(outcome, Ok(Outcome::Exited));
  assert!(
    begun.elapsed() >= Duration::from_secs(2),
    "{:?}",
    begun.elapsed()
  );
}
