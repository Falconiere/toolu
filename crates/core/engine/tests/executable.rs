//! `.sh` registry modules (AC-2, AC-3): the stdin, environment and exit-status
//! contract of `dispatch.sh`, and the deadline and isolation TypeScript lacks.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::time::{Duration, Instant};

use hook::Hook;
use toolu_engine::Phase;
use toolu_engine::trace::StepStatus;
use toolu_protocol::host::Host;

const BASH: &str =
  r#"{"tool_name":"Bash","tool_input":{"command":"git status"},"session_id":"s1"}"#;

fn advice(text: &str) -> String {
  format!(
    r#"printf '%s\n' '{{"hookSpecificOutput":{{"hookEventName":"PreToolUse","additionalContext":"{text}"}}}}'"#
  )
}

#[test]
fn a_module_gets_the_payload_and_its_environment_and_exit_2_blocks() {
  let hook = Hook::new(Host::Claude).unwrap();
  let probe = r#"s=$(cat; printf x); s=${s%x}
printf '%s|%s|%s|%s|%s|%s\n' "$s" "$input" "$tool_name" "$TOOLU_LIB_DIR" "$TOOLU_CONFIG_DIR" "${TOOLU_EDIT_OPERATION-unset}" >&2
exit 2"#;
  hook.sh(Phase::Pre, "x@t__probe.sh", probe).unwrap();
  hook
    .sh(Phase::Pre, "x@t__later.sh", &advice("never"))
    .unwrap();
  let out = hook
    .run(Phase::Pre, &format!("{BASH}\n\n"), &[], &[])
    .result;
  let expected = format!(
    "{BASH}\n|{BASH}|Bash|{}|{}|unset\n",
    hook.sb.text("plugin/hooks/lib"),
    hook.sb.text("home/.claude")
  );
  assert_eq!(
    (out.stdout.as_str(), out.stderr.as_str(), out.exit_code),
    ("", expected.as_str(), 2)
  );
}

#[test]
fn other_failures_are_reported_their_stderr_dropped_and_the_walk_goes_on() {
  let hook = Hook::new(Host::Claude).unwrap();
  hook
    .sh(Phase::Pre, "x@t__a.sh", "printf 'dropped\\n' >&2\nexit 3")
    .unwrap();
  hook
    .sh(Phase::Pre, "x@t__b.sh", "printf 'also dropped\\n' >&2")
    .unwrap();
  hook.sh(Phase::Pre, "x@t__c.sh", "kill -SEGV $$").unwrap();
  hook.sh(Phase::Pre, "x@t__d.sh", &advice("after")).unwrap();
  let dispatched = hook.run(Phase::Pre, BASH, &[], &[]);
  let out = dispatched.result;
  assert_eq!(
    out.stderr,
    "toolu-dispatch: module x@t__a.sh exited 3; output skipped\ntoolu-dispatch: module x@t__c.sh exited 139; output skipped\n"
  );
  assert!(
    out.stdout.contains("\"additionalContext\": \"after\""),
    "{}",
    out.stdout
  );
  assert_eq!(out.exit_code, 0);
  let statuses: Vec<&StepStatus> = dispatched.trace.iter().map(|step| &step.status).collect();
  assert_eq!(
    statuses,
    [
      &StepStatus::Exited(3),
      &StepStatus::Exited(0),
      &StepStatus::Exited(139),
      &StepStatus::Exited(0)
    ]
  );
}

#[test]
fn a_module_past_its_deadline_is_killed_and_reported_never_denying() {
  let mut hook = Hook::new(Host::Claude).unwrap();
  hook.timeout = Duration::from_secs(1);
  let deny = r#"printf '%s\n' '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"late"}}'"#;
  hook
    .sh(Phase::Pre, "x@t__a.sh", &format!("sleep 5\n{deny}"))
    .unwrap();
  hook
    .sh(Phase::Pre, "x@t__b.sh", "trap '' TERM\nsleep 5\nexit 2")
    .unwrap();
  hook
    .sh(Phase::Pre, "x@t__c.sh", &advice("still runs"))
    .unwrap();
  let started = Instant::now();
  let out = hook.run(Phase::Pre, BASH, &[], &[]).result;
  assert_eq!(
    out.stderr,
    "toolu-dispatch: module x@t__a.sh exited 124; output skipped\ntoolu-dispatch: module x@t__b.sh exited 124; output skipped\n"
  );
  assert!(out.stdout.contains("still runs"), "{}", out.stdout);
  assert_eq!(out.exit_code, 0);
  assert!(
    started.elapsed() < Duration::from_secs(8),
    "{:?}",
    started.elapsed()
  );
}

#[test]
fn a_backgrounded_process_with_redirected_streams_is_not_waited_for() {
  let mut hook = Hook::new(Host::Claude).unwrap();
  hook.timeout = Duration::from_secs(3);
  let body = format!("sleep 30 >/dev/null 2>&1 &\n{}", advice("backgrounded"));
  hook.sh(Phase::Pre, "x@t__a.sh", &body).unwrap();
  let out = hook.run(Phase::Pre, BASH, &[], &[]).result;
  assert_eq!(out.stderr, "");
  assert!(out.stdout.contains("backgrounded"), "{}", out.stdout);
}

#[test]
fn output_past_the_budget_is_skipped_unless_the_module_denies() {
  let hook = Hook::new(Host::Claude).unwrap();
  hook
    .sh(
      Phase::Pre,
      "x@t__a.sh",
      "head -c 9000000 /dev/zero | tr '\\0' x",
    )
    .unwrap();
  let out = hook.run(Phase::Pre, BASH, &[], &[]).result;
  assert_eq!(
    (out.stderr.as_str(), out.exit_code),
    (
      "toolu-dispatch: module x@t__a.sh printed more than 8388608 bytes; output skipped\n",
      0
    )
  );
  hook
    .sh(
      Phase::Pre,
      "x@t__b.sh",
      "head -c 9000000 /dev/zero | tr '\\0' y >&2\nexit 2",
    )
    .unwrap();
  let out = hook.run(Phase::Pre, BASH, &[], &[]).result;
  assert_eq!(out.exit_code, 2);
  assert!(
    out.stderr.ends_with("yyyy"),
    "the deny keeps its stderr, cut at the budget"
  );
}

#[test]
fn a_module_bash_cannot_run_is_skipped_as_127() {
  let mut hook = Hook::new(Host::Claude).unwrap();
  hook
    .sh(Phase::Pre, "x@t__a.sh", &advice("unreachable"))
    .unwrap();
  hook
    .extra
    .push(("PATH".to_owned(), hook.sb.text("empty-bin")));
  let out = hook.run(Phase::Pre, BASH, &[], &[]).result;
  assert_eq!(
    (out.stdout.as_str(), out.stderr.as_str()),
    (
      "",
      "toolu-dispatch: module x@t__a.sh exited 127; output skipped\n"
    )
  );
}
