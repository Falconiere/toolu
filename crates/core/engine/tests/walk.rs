//! The walk (AC-4, AC-10, two-phase ordering): built-ins fold as they decide,
//! the registry phase runs before its results fold, a built-in's failure is an
//! exit 1, shadowing and selection skip modules, and a patch folds per path.

#[path = "helpers/expect.rs"]
mod expect;
#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/modules.rs"]
mod modules;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::collections::BTreeSet;

use hook::Hook;
use toolu_engine::Phase;
use toolu_engine::gate::Gate;
use toolu_engine::trace::{Skip, StepKind, StepStatus};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::registry::rule::RuleContext;

/// A built-in with a fixed answer.
struct Fixed(&'static str, Result<Decision, String>);

impl Gate for Fixed {
  fn name(&self) -> &str {
    self.0
  }

  fn run(&self, _event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Result<Decision, String> {
    self.1.clone()
  }
}

const BASH: &str = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"session_id":"s1"}"#;

fn ask(reason: &str) -> String {
  format!(
    r#"printf '%s\n' '{{"hookSpecificOutput":{{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"{reason}"}}}}'"#
  )
}

#[test]
fn registry_warnings_come_before_registry_exit_lines_as_in_typescript() {
  let hook = Hook::new(Host::Claude).unwrap();
  let pre = Phase::Pre;
  hook.sh(pre, "a@t__first.sh", "exit 1").unwrap();
  modules::js(&hook, pre, "b@t", "boom", r#"throw new Error("boom");"#).unwrap();
  modules::file(&hook, pre, "noname.sh", "exit 2").unwrap();
  let boom = Fixed("native", Err("exploded".to_owned()));
  let dispatched = hook.run(pre, BASH, &[&boom], &[]);
  assert_eq!(
    dispatched.result.stderr,
    "toolu-dispatch: module native exited 1; output skipped\n\
     toolu-registry: registry module noname.sh lacks <plugin-spec>__<name> namespace; skipped\n\
     toolu-registry: module b@t__boom.js failed: boom; output skipped\n\
     toolu-dispatch: module a@t__first.sh exited 1; output skipped\n"
  );
  assert_eq!(dispatched.result.exit_code, 0);
  let first = dispatched.trace.first().unwrap();
  assert_eq!(
    (first.kind, &first.status),
    (
      StepKind::Builtin,
      &StepStatus::Failed("exploded".to_owned())
    )
  );
}

#[test]
fn a_built_in_deny_wins_before_a_registry_ask_runs() {
  let hook = Hook::new(Host::Claude).unwrap();
  hook
    .sh(
      Phase::Pre,
      "comemory@comemory__recall.sh",
      &ask("recall first?"),
    )
    .unwrap();
  let deny = Fixed(
    "gate",
    Ok(Decision::Deny {
      reason: Text::new("built-in denies").unwrap(),
    }),
  );
  let dispatched = hook.run(Phase::Pre, BASH, &[&deny], &[]);
  assert_eq!(
    dispatched.result.stdout,
    "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"built-in denies\"}}\n"
  );
  assert_eq!(dispatched.trace.len(), 1, "the registry never ran");
}

#[test]
fn a_js_module_shadows_its_specs_sh_and_selection_gates_specs() {
  let mut hook = Hook::new(Host::Claude).unwrap();
  let pre = Phase::Pre;
  hook.sh(pre, "x@t__old.sh", "exit 2").unwrap();
  modules::js(
    &hook,
    pre,
    "x@t",
    "new",
    r#"return { kind: "advisory", message: "fresh js" };"#,
  )
  .unwrap();
  hook
    .sh(
      pre,
      "y@t__other.sh",
      r#"printf '%s\n' '{"systemMessage":"other spec"}'"#,
    )
    .unwrap();
  modules::manifest(&hook, pre, "z@t", "w", "Write").unwrap();
  modules::install(&hook, &["x@t", "y@t"]).unwrap();
  let dispatched = hook.run(pre, BASH, &[], &[]);
  let both = expect::merged("PreToolUse", Some("fresh js"), Some("other spec"));
  assert_eq!(dispatched.result.stdout, both);
  let old = dispatched
    .trace
    .iter()
    .find(|step| step.module == "x@t__old.sh")
    .unwrap();
  assert_eq!(old.status, StepStatus::Skipped(Skip::Shadowed));
  hook.selected = Some(BTreeSet::from(["y@t".to_owned()]));
  let dispatched = hook.run(pre, BASH, &[], &[]);
  let selected = expect::merged("PreToolUse", None, Some("other spec"));
  assert_eq!(dispatched.result.stdout, selected);
  let skipped = dispatched
    .trace
    .iter()
    .filter(|step| step.status == StepStatus::Skipped(Skip::Inactive))
    .count();
  assert_eq!(skipped, 3, "x@t's two modules and the uninstalled z@t");
}

#[test]
fn an_ask_from_one_patch_path_gets_the_other_paths_advice() {
  let hook = Hook::new(Host::Claude).unwrap();
  let one = hook.sb.text("project/one.ts");
  let two = hook.sb.text("project/two.ts");
  let body = format!(
    "case \"$TOOLU_EDIT_OPERATION:$(jq -r .tool_input.file_path <<<\"$input\")\" in\n  add:{one}) {};;\n  *) printf '%s\\n' '{{\"systemMessage\":\"second path\"}}';;\nesac",
    ask("first path asks")
  );
  hook.sh(Phase::Pre, "x@t__a.sh", &body).unwrap();
  let patch = format!(
    r#"{{"tool_name":"apply_patch","tool_input":{{"command":"*** Begin Patch\n*** Add File: {one}\n+a\n*** Update File: {two}\n@@\n-a\n+b\n*** End Patch"}}}}"#
  );
  let out = hook.run(Phase::Pre, &patch, &[], &[]).result;
  let doc: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
  assert_eq!(doc["hookSpecificOutput"]["permissionDecision"], "ask");
  assert_eq!(
    doc["hookSpecificOutput"]["permissionDecisionReason"],
    "first path asks"
  );
  assert_eq!(doc["systemMessage"], "second path");
}

#[test]
fn after_a_tool_blocks_from_every_path_can_be_collected() {
  let mut hook = Hook::new(Host::Claude).unwrap();
  let body = r#"jq -cn --arg f "$(jq -r .tool_input.file_path <<<"$input")" '{decision: "block", reason: ("invalid " + $f)}'"#;
  hook.sh(Phase::Post, "x@t__a.sh", body).unwrap();
  let (one, two) = (
    hook.sb.text("project/one.ts"),
    hook.sb.text("project/two.ts"),
  );
  let patch = format!(
    r#"{{"tool_name":"apply_patch","tool_input":{{"command":"*** Begin Patch\n*** Add File: {one}\n+a\n*** Add File: {two}\n+b\n*** End Patch"}}}}"#
  );
  let first = hook.run(Phase::Post, &patch, &[], &[]).result;
  assert_eq!(
    first.stdout,
    format!("{{\"decision\":\"block\",\"reason\":\"invalid {one}\"}}\n")
  );
  hook.continue_blocks = true;
  let all = hook.run(Phase::Post, &patch, &[], &[]).result;
  assert_eq!(
    all.stdout,
    format!("{{\"decision\":\"block\",\"reason\":\"invalid {one}\\n\\ninvalid {two}\"}}\n")
  );
}

#[test]
fn on_codex_a_built_in_ask_is_a_deny() {
  let hook = Hook::new(Host::Codex).unwrap();
  let ask = Fixed(
    "a",
    Ok(Decision::Ask {
      reason: Text::new("confirm?").unwrap(),
    }),
  );
  let out = hook.run(Phase::Pre, BASH, &[&ask], &[]).result;
  let doc: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
  assert_eq!(doc["hookSpecificOutput"]["permissionDecision"], "deny");
}
