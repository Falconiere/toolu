//! Compiled-in rules behind manifests (AC-5, AC-9): only a manifest whose matcher
//! fits the walk's tool, naming a rule this binary has, whose `applies` holds,
//! runs its rule; a bad manifest is one stderr line; a usable manifest shadows
//! its spec's older modules; and a walk with nothing to run spawns nothing.

#[path = "helpers/bins.rs"]
mod bins;
#[path = "helpers/counting.rs"]
mod counting;
#[path = "helpers/expect.rs"]
mod expect;
#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/modules.rs"]
mod modules;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use counting::{Counting, payload};
use hook::Hook;
use toolu_engine::Phase;
use toolu_engine::trace::{Skip, StepStatus};
use toolu_protocol::host::Host;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::Rule;

/// A hook whose `bash` and `bun` log each spawn to the returned file, then run the real ones.
fn sentinels() -> sandbox::Res<(Hook, std::path::PathBuf)> {
  let mut hook = Hook::new(Host::Claude)?;
  let log = hook.sb.path("spawned");
  for name in ["bash", "bun"] {
    let real = bins::which(name)?;
    let body = format!("echo {name} >> {}\nexec {real} \"$@\"", log.display());
    bins::script(&hook.sb.path(&format!("bin/{name}")), &body)?;
  }
  let path = std::env::var("PATH").map_err(|err| err.to_string())?;
  hook
    .extra
    .push(("PATH".to_owned(), format!("{}:{path}", hook.sb.text("bin"))));
  Ok((hook, log))
}

#[test]
fn a_read_with_no_fitting_manifest_runs_no_rule_and_spawns_nothing() {
  let (hook, log) = sentinels().unwrap();
  modules::manifest(&hook, Phase::Pre, "x@t", "r", "Write|Edit").unwrap();
  modules::manifest(&hook, Phase::Pre, "y@t", "s", "mcp__*").unwrap();
  let r = Counting::new("x@t", "r", RegistryEvent::ToolPre);
  let s = Counting::new("y@t", "s", RegistryEvent::ToolPre);
  let read = payload("Read", r#"{"file_path":"/etc/hosts"}"#);
  let dispatched = hook.run(Phase::Pre, &read, &[], &[&r, &s]);
  assert_eq!(dispatched.result.stdout, "");
  assert_eq!(dispatched.result.stderr, "");
  assert_eq!((r.calls(), s.calls()), ((0, 0), (0, 0)));
  assert!(
    dispatched
      .trace
      .iter()
      .all(|step| step.status == StepStatus::Skipped(Skip::NotMatching))
  );
  assert_eq!(dispatched.trace.len(), 2);
  assert!(!log.exists(), "no module process may be spawned");
  modules::manifest(&hook, Phase::Pre, "x@t", "r", "*").unwrap();
  let out = hook.run(Phase::Pre, &read, &[], &[&r, &s]).result;
  assert_eq!(
    out.stdout,
    expect::merged("PreToolUse", Some("r saw Read"), None),
    "a `*` manifest runs"
  );
  assert_eq!(r.calls(), (1, 1));
  assert!(!log.exists());
  hook.sh(Phase::Pre, "w@t__probe.sh", "exit 0").unwrap();
  modules::js(
    &hook,
    Phase::Pre,
    "z@t",
    "probe",
    r#"return { kind: "allow" };"#,
  )
  .unwrap();
  hook.run(Phase::Pre, &read, &[], &[&r, &s]);
  let spawned = std::fs::read_to_string(&log).unwrap();
  assert_eq!(
    spawned, "bash\nbun\n",
    "control: the sentinels see a .sh and a .js module run"
  );
}

#[test]
fn edits_reach_rules_as_edit_and_matchers_fit_names_and_prefixes() {
  let hook = Hook::new(Host::Claude).unwrap();
  modules::manifest(&hook, Phase::Pre, "e@t", "edit", "Edit").unwrap();
  modules::manifest(&hook, Phase::Pre, "w@t", "write", "Write").unwrap();
  modules::manifest(&hook, Phase::Pre, "m@t", "mcp", "Bash|mcp__*").unwrap();
  let edit = Counting::new("e@t", "edit", RegistryEvent::ToolPre);
  let write = Counting::new("w@t", "write", RegistryEvent::ToolPre);
  let mcp = Counting::new("m@t", "mcp", RegistryEvent::ToolPre);
  let rules: [&dyn Rule; 3] = [&edit, &write, &mcp];
  let file = hook.sb.text("project/a.ts");
  let out = hook
    .run(
      Phase::Pre,
      &payload(
        "Write",
        &format!(r#"{{"file_path":"{file}","content":"x"}}"#),
      ),
      &[],
      &rules,
    )
    .result;
  assert_eq!(
    out.stdout,
    expect::merged("PreToolUse", Some("edit saw Edit"), None)
  );
  let patch =
    format!(r#"{{"command":"*** Begin Patch\n*** Add File: {file}\n+x\n*** End Patch"}}"#);
  hook.run(Phase::Pre, &payload("apply_patch", &patch), &[], &rules);
  assert_eq!((edit.calls(), write.calls()), ((2, 2), (0, 0)));
  let out = hook
    .run(Phase::Pre, &payload("mcp__x__y", "{}"), &[], &rules)
    .result;
  assert_eq!(
    out.stdout,
    expect::merged("PreToolUse", Some("mcp saw mcp__x__y"), None)
  );
  hook.run(Phase::Pre, &payload("mcp_x", "{}"), &[], &rules);
  assert_eq!(mcp.calls(), (1, 1));
}

#[test]
fn a_rule_that_does_not_apply_is_not_run() {
  let hook = Hook::new(Host::Claude).unwrap();
  modules::manifest(&hook, Phase::Post, "ts-quality@toolu", "ts-quality", "Edit").unwrap();
  let mut rule = Counting::new("ts-quality@toolu", "ts-quality", RegistryEvent::ToolPost);
  rule.extension = Some("ts");
  let edit = |name: &str| {
    payload(
      "Edit",
      &format!(
        r#"{{"file_path":"{}","old_string":"a","new_string":"b"}}"#,
        hook.sb.text(name)
      ),
    )
  };
  let dispatched = hook.run(Phase::Post, &edit("project/a.py"), &[], &[&rule]);
  assert_eq!(rule.calls(), (1, 0), "a .py edit leaves ts-quality unrun");
  let statuses: Vec<StepStatus> = dispatched
    .trace
    .into_iter()
    .map(|step| step.status)
    .collect();
  assert_eq!(statuses, [StepStatus::Skipped(Skip::NotMatching)]);
  let out = hook
    .run(Phase::Post, &edit("project/a.ts"), &[], &[&rule])
    .result;
  assert_eq!(rule.calls(), (2, 1));
  assert_eq!(
    out.stdout,
    expect::merged("PostToolUse", Some("ts-quality saw Edit"), None)
  );
}

#[test]
fn manifest_problems_are_one_line_each_and_the_walk_goes_on() {
  let hook = Hook::new(Host::Claude).unwrap();
  let pre = Phase::Pre;
  modules::file(
    &hook,
    pre,
    "a@t__v2.json",
    r#"{"version":2,"spec":"a@t","name":"v2","event":"tool/pre","matcher":"Write"}"#,
  )
  .unwrap();
  modules::file(
    &hook,
    pre,
    "b@t__extra.json",
    r#"{"version":1,"spec":"b@t","name":"extra","event":"tool/pre","matcher":"*","x":1}"#,
  )
  .unwrap();
  modules::manifest(&hook, pre, "c@t", "gone", "Bash").unwrap();
  modules::manifest(&hook, pre, "d@t", "gone", "Write").unwrap();
  hook
    .sh(
      pre,
      "e@t__after.sh",
      r#"printf '%s\n' '{"systemMessage":"after"}'"#,
    )
    .unwrap();
  let out = hook
    .run(pre, &payload("Bash", r#"{"command":"ls"}"#), &[], &[])
    .result;
  let version = env!("CARGO_PKG_VERSION");
  assert_eq!(
    out.stderr,
    format!(
      "toolu-registry: manifest a@t__v2.json skipped: unsupported version 2 (supported: 1)\n\
       toolu-registry: manifest b@t__extra.json skipped: unknown field `x`, expected one of `version`, `spec`, `name`, `event`, `matcher` at line 1 column 77\n\
       toolu-registry: manifest c@t__gone.json skipped: no rule c@t__gone in toolu {version}\n"
    )
  );
  assert_eq!(
    out.stdout,
    expect::merged("PreToolUse", None, Some("after")),
    "a bad manifest, even one whose matcher does not fit, is reported and the walk goes on"
  );
}

#[test]
fn a_usable_manifest_shadows_its_specs_modules_and_gating_still_applies() {
  let hook = Hook::new(Host::Claude).unwrap();
  let pre = Phase::Pre;
  modules::manifest(&hook, pre, "x@t", "r", "*").unwrap();
  modules::js(
    &hook,
    pre,
    "x@t",
    "old",
    r#"return { kind: "deny", reason: "stale js" };"#,
  )
  .unwrap();
  hook.sh(pre, "x@t__old.sh", "exit 2").unwrap();
  modules::manifest(&hook, pre, "z@t", "off", "*").unwrap();
  modules::install(&hook, &["x@t"]).unwrap();
  let rule = Counting::new("x@t", "r", RegistryEvent::ToolPre);
  let off = Counting::new("z@t", "off", RegistryEvent::ToolPre);
  let bash = payload("Bash", r#"{"command":"ls"}"#);
  let dispatched = hook.run(pre, &bash, &[], &[&rule, &off]);
  let advice = expect::merged("PreToolUse", Some("r saw Bash"), None);
  assert_eq!(dispatched.result.stdout, advice);
  let steps: Vec<String> = dispatched
    .trace
    .iter()
    .map(|step| format!("{} {:?} {:?}", step.module, step.kind, step.status))
    .collect();
  assert_eq!(
    steps,
    [
      "x@t__old.js Esm Skipped(Shadowed)",
      "x@t__old.sh Executable Skipped(Shadowed)",
      "x@t__r.json Rule Decided",
      "z@t__off.json Rule Skipped(Inactive)",
    ]
  );
  assert_eq!(off.calls(), (0, 0));
}

#[test]
fn a_usable_manifest_shadows_whatever_the_tool() {
  let hook = Hook::new(Host::Claude).unwrap();
  modules::manifest(&hook, Phase::Pre, "v@t", "w", "Write").unwrap();
  modules::js(
    &hook,
    Phase::Pre,
    "v@t",
    "old",
    r#"return { kind: "deny", reason: "stale" };"#,
  )
  .unwrap();
  let write = Counting::new("v@t", "w", RegistryEvent::ToolPre);
  let dispatched = hook.run(
    Phase::Pre,
    &payload("Bash", r#"{"command":"ls"}"#),
    &[],
    &[&write],
  );
  assert_eq!(dispatched.result.stdout, "", "the stale .js never ran");
  let steps: Vec<String> = dispatched
    .trace
    .iter()
    .map(|step| format!("{} {:?}", step.module, step.status))
    .collect();
  assert_eq!(
    steps,
    [
      "v@t__old.js Skipped(Shadowed)",
      "v@t__w.json Skipped(NotMatching)"
    ]
  );
  assert_eq!(write.calls(), (0, 0));
}
