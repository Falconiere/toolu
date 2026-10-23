use std::time::Duration;

use super::{AgentPlan, Resolved, ensure_agent};
use crate::herdr::Call;
use crate::hosts::HostKind;
use crate::launch_rig::Rig;

fn plan(pane: &str, worktree: &str) -> AgentPlan {
  AgentPlan {
    key: "k-1".into(),
    pane: pane.into(),
    dry: false,
    replace: false,
    host: Resolved {
      kind: HostKind::Claude,
      model: None,
      effort: None,
    },
    bypass: true,
    permission_mode: "auto".into(),
    previous_kind: None,
    launches: 0,
    worktree: worktree.into(),
    session_id: None,
    session_started_after: None,
  }
}

#[test]
fn a_dry_start_prints_the_argv_and_a_resume_needs_its_session() {
  let rig = Rig::new();
  let mut ctx = rig.ctx();
  let mut log = Vec::new();
  let dry = AgentPlan {
    dry: true,
    ..plan("<root-pane>", "/wt")
  };
  let start = ensure_agent(&mut ctx.herdr, &dry, &mut log, Duration::from_secs(5)).expect("dry");
  assert_eq!(
    log,
    [
      "herdr agent start k-1 --kind claude --pane '<root-pane>' --timeout 90000 -- --dangerously-skip-permissions -n k-1"
    ]
  );
  assert!(start.started && !start.resumed);
  let resume = AgentPlan {
    launches: 1,
    previous_kind: Some("claude".into()),
    ..dry.clone()
  };
  let missing =
    ensure_agent(&mut ctx.herdr, &resume, &mut log, Duration::from_secs(5)).expect_err("session");
  assert!(missing.message.starts_with("captured session ID missing"));
  let resumed = AgentPlan {
    session_id: Some("0199a272-0b4e-7cc3-8c4f-071c81275655".into()),
    ..resume
  };
  let start =
    ensure_agent(&mut ctx.herdr, &resumed, &mut log, Duration::from_secs(5)).expect("resume");
  assert!(start.resumed && start.args.starts_with(&["--resume".to_owned()]));
}

#[test]
fn a_live_agent_is_kept_only_with_its_recorded_session_and_host() {
  let rig = Rig::new();
  let worktree = rig.tmp.path().join("wt");
  std::fs::create_dir_all(&worktree).expect("worktree");
  let pane = format!("{}:p1", rig.herdr.add_repo(&worktree));
  let worktree = std::fs::canonicalize(&worktree)
    .expect("real")
    .display()
    .to_string();
  let mut ctx = rig.ctx();
  let start = Call::Start {
    name: "k-1".into(),
    kind: "claude".into(),
    pane_id: pane.clone(),
    args: Vec::new(),
    timeout_ms: None,
  };
  ctx.herdr.call(&start).expect("start");
  let live = plan(&pane, &worktree);
  let wait = Duration::from_secs(5);
  let unrecorded = ensure_agent(&mut ctx.herdr, &live, &mut Vec::new(), wait).expect_err("session");
  assert_eq!(
    unrecorded.message,
    "live session identity is missing; inspect before explicit --replace"
  );
  let moving = AgentPlan {
    host: Resolved {
      kind: HostKind::Codex,
      model: None,
      effort: None,
    },
    previous_kind: Some("claude".into()),
    ..live.clone()
  };
  let refused = ensure_agent(&mut ctx.herdr, &moving, &mut Vec::new(), wait).expect_err("move");
  assert_eq!(refused.message, "host change requires explicit --replace");
  let replace = AgentPlan {
    replace: true,
    ..moving
  };
  let mut log = Vec::new();
  let started = ensure_agent(&mut ctx.herdr, &replace, &mut log, wait).expect("replaced");
  assert!(started.started && log[0] == "herdr agent prompt k-1 /exit");
  assert!(log[1].starts_with("herdr agent start k-1 --kind codex"));
}
