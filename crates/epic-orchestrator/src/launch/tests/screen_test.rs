use super::{Left, approval_screen, explain};
use crate::herdr_state::Scripted;
use crate::hosts::HostKind;
use crate::launch::failure::{APPROVAL, Failure};
use crate::launch::{find_issue, launch_issue};
use crate::launch_rig::{Rig, opts};

/// Codex 0.164.0's startup modal, captured from a fresh `CODEX_HOME` (#626).
const MODAL: &str = "  Hooks need review
  18 hooks are new or changed.
  Hooks can run outside the sandbox after you trust them.

› 1. Review hooks
  2. Trust all and continue
  3. Continue without trusting (hooks won't run)

  enter confirm · esc skip";

/// The hooks browser the modal's Enter opens: what the issue's workers sat on.
const BROWSER: &str = "  Hooks
  Lifecycle hooks from config and enabled plugins.

  ⚠ 26 hooks need review before they can run.

  Event                 Installed   Active      Review      Description

› PreToolUse            3           0           3           Before a tool executes
  SessionStart          11          0           11          When a new session starts
↓

  t trust all · enter review · esc close";

const COMPOSER: &str =
  "› Ask Codex to do anything\n\n  GPT-6.1-Sol default · /tmp/wt2\n  ? for shortcuts";

#[test]
fn codex_hook_review_screens_are_named_by_their_line() {
  assert_eq!(
    approval_screen(HostKind::Codex, MODAL).as_deref(),
    Some("18 hooks are new or changed.")
  );
  assert_eq!(
    approval_screen(HostKind::Codex, BROWSER).as_deref(),
    Some("26 hooks need review before they can run.")
  );
  assert_eq!(
    approval_screen(HostKind::Codex, "1 hook needs review before it can run.").as_deref(),
    Some("1 hook needs review before it can run.")
  );
  assert_eq!(
    approval_screen(HostKind::Codex, "  1 hook is new or changed.").as_deref(),
    Some("1 hook is new or changed.")
  );
  assert_eq!(approval_screen(HostKind::Codex, COMPOSER), None);
  assert_eq!(approval_screen(HostKind::Claude, BROWSER), None);
}

fn lost() -> Failure {
  Failure::unknown("cannot attest codex session identity; inspect the owned pane before retry")
}

fn left(kind: HostKind) -> Left<'static> {
  Left {
    name: "k-1",
    kind,
    pane: Some("w1:p1"),
  }
}

#[test]
fn a_failure_over_a_hook_review_needs_a_human_and_names_the_screen_and_pane() {
  let rig = Rig::new();
  BROWSER.clone_into(&mut rig.herdr.state().tail);
  let mut ctx = rig.ctx();
  let named = explain(lost(), &mut ctx.herdr, &left(HostKind::Codex));
  assert_eq!(named.code.as_deref(), Some(APPROVAL));
  assert!(named.uncertain);
  assert_eq!(
    named.message,
    "codex is waiting on its hook review in pane w1:p1: \"26 hooks need review before they can run.\"; review and trust the hooks there, then relaunch with --replace"
  );
  assert_eq!(named.details()["lifecycle_outcome"], "needs-human");
}

#[test]
fn a_failure_without_a_screen_or_with_a_limit_is_kept() {
  let rig = Rig::new();
  COMPOSER.clone_into(&mut rig.herdr.state().tail);
  let mut ctx = rig.ctx();
  assert_eq!(
    explain(lost(), &mut ctx.herdr, &left(HostKind::Codex)),
    lost()
  );
  format!("{BROWSER}\nYou've hit your usage limit.").clone_into(&mut rig.herdr.state().tail);
  assert_eq!(
    explain(lost(), &mut ctx.herdr, &left(HostKind::Codex)),
    lost()
  );
  BROWSER.clone_into(&mut rig.herdr.state().tail);
  assert_eq!(
    explain(lost(), &mut ctx.herdr, &left(HostKind::Claude)),
    lost()
  );
  assert_eq!(lost().details()["lifecycle_outcome"], "uncertain");
}

#[test]
fn a_codex_launch_stuck_on_its_hook_review_records_needs_human() {
  let mut rig = Rig::new();
  rig.checkout("comemory-255");
  BROWSER.clone_into(&mut rig.herdr.state().tail);
  let mut flags = opts();
  flags.force = true;
  flags.kind = Some("codex".into());
  let issue = find_issue(&rig.graph, "comemory-255")
    .expect("issue")
    .clone();
  let (_github, client) = crate::support::comemory_github();
  let mut ctx = rig.ctx();
  ctx.gh = Ok(&client);
  ctx.session_wait = std::time::Duration::from_millis(600);
  let err = launch_issue(&rig.graph, &issue, &flags, &mut ctx).expect_err("no codex session");
  assert_eq!(err.code.as_deref(), Some(APPROVAL), "{}", err.message);
  assert!(
    err
      .message
      .starts_with("codex is waiting on its hook review in pane "),
    "{}",
    err.message
  );
  let record = rig.record("comemory-255");
  assert_eq!(record["lifecycle_outcome"], "needs-human");
  assert_eq!(record["stage"], "uncertain");
  assert_eq!(rig.leases(), 1);
}

#[test]
fn a_refusal_or_limit_is_kept_unread_and_an_unreadable_pane_keeps_the_failure() {
  let rig = Rig::new();
  BROWSER.clone_into(&mut rig.herdr.state().tail);
  let mut ctx = rig.ctx();
  let busy = Failure {
    code: Some(crate::launch::PANE_BUSY.to_owned()),
    ..Failure::refusal("agent target pane w1:p1 is not an available shell")
  };
  assert_eq!(
    explain(busy.clone(), &mut ctx.herdr, &left(HostKind::Codex)),
    busy
  );
  let limited = Failure::unknown("You've hit your usage limit.");
  assert_eq!(
    explain(limited.clone(), &mut ctx.herdr, &left(HostKind::Codex)),
    limited
  );
  assert!(
    !rig
      .herdr
      .state()
      .calls
      .iter()
      .any(|(method, _params)| method == "agent.read"),
    "a refusal or limit read the pane"
  );
  rig.herdr.script(
    "agent.read",
    Scripted::Error {
      code: "agent_not_found".into(),
      message: "no agent k-1".into(),
    },
  );
  assert_eq!(
    explain(lost(), &mut ctx.herdr, &left(HostKind::Codex)),
    lost()
  );
}

#[test]
fn an_unrecorded_pane_is_named_as_such() {
  let rig = Rig::new();
  MODAL.clone_into(&mut rig.herdr.state().tail);
  let mut ctx = rig.ctx();
  let unrecorded = Left {
    pane: None,
    ..left(HostKind::Codex)
  };
  assert_eq!(
    explain(lost(), &mut ctx.herdr, &unrecorded).message,
    "codex is waiting on its hook review in pane (unrecorded): \"18 hooks are new or changed.\"; review and trust the hooks there, then relaunch with --replace"
  );
}
