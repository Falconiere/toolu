//! Compiled-in rules behind manifests (AC-5, AC-9): only a manifest whose matcher
//! fits the walk's tool, naming a rule this binary has, whose `applies` holds,
//! runs its rule; a bad manifest is one stderr line; a usable manifest shadows
//! its spec's older modules; and a walk with nothing to run spawns nothing.

#[path = "helpers/bins.rs"]
mod bins;
#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/modules.rs"]
mod modules;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use hook::Hook;
use toolu_engine::Phase;
use toolu_engine::trace::{Skip, StepStatus};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};

/// A rule that counts its `applies` and `run` calls and advises its name.
struct Counting {
  spec: &'static str,
  name: &'static str,
  event: RegistryEvent,
  /// Only paths with this extension apply; every event when `None`.
  extension: Option<&'static str>,
  checks: AtomicUsize,
  runs: AtomicUsize,
}

impl Counting {
  fn new(spec: &'static str, name: &'static str, event: RegistryEvent) -> Counting {
    Counting {
      spec,
      name,
      event,
      extension: None,
      checks: AtomicUsize::new(0),
      runs: AtomicUsize::new(0),
    }
  }

  fn calls(&self) -> (usize, usize) {
    (
      self.checks.load(Ordering::SeqCst),
      self.runs.load(Ordering::SeqCst),
    )
  }
}

impl Rule for Counting {
  fn spec(&self) -> &str {
    self.spec
  }

  fn name(&self) -> &str {
    self.name
  }

  fn event(&self) -> RegistryEvent {
    self.event
  }

  fn applies(&self, _event: &NormalizedEvent, ctx: &RuleContext<'_>) -> bool {
    self.checks.fetch_add(1, Ordering::SeqCst);
    let path = ctx
      .raw
      .get("tool_input")
      .and_then(|input| input.get("file_path")?.as_str());
    self
      .extension
      .is_none_or(|ext| path.is_some_and(|path| Path::new(path).extension() == Some(ext.as_ref())))
  }

  fn run(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Decision {
    self.runs.fetch_add(1, Ordering::SeqCst);
    let tool = event
      .tool()
      .map(|tool| tool.name.as_str().to_owned())
      .unwrap_or_default();
    Text::new(format!("{} saw {tool}", self.name))
      .map_or(Decision::Allow, |message| Decision::Advisory { message })
  }
}

fn payload(tool: &str, input: &str) -> String {
  format!(r#"{{"tool_name":"{tool}","tool_input":{input},"session_id":"s1"}}"#)
}

#[test]
fn a_read_with_no_fitting_manifest_runs_no_rule_and_spawns_nothing() {
  let mut hook = Hook::new(Host::Claude).unwrap();
  let log = hook.sb.path("spawned");
  for name in ["bash", "bun"] {
    let real = bins::which(name).unwrap_or_else(|_| "/bin/false".to_owned());
    let body = format!("echo {name} >> {}\nexec {real} \"$@\"", log.display());
    bins::script(&hook.sb.path(&format!("bin/{name}")), &body).unwrap();
  }
  let path = std::env::var("PATH").unwrap();
  hook
    .extra
    .push(("PATH".to_owned(), format!("{}:{path}", hook.sb.text("bin"))));
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
  assert!(
    out.stdout.contains("r saw Read"),
    "a `*` manifest runs: {}",
    out.stdout
  );
  assert_eq!(r.calls(), (1, 1));
  assert!(!log.exists());
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
  assert!(out.stdout.contains("edit saw Edit"), "{}", out.stdout);
  let patch =
    format!(r#"{{"command":"*** Begin Patch\n*** Add File: {file}\n+x\n*** End Patch"}}"#);
  hook.run(Phase::Pre, &payload("apply_patch", &patch), &[], &rules);
  assert_eq!((edit.calls(), write.calls()), ((2, 2), (0, 0)));
  let out = hook
    .run(Phase::Pre, &payload("mcp__x__y", "{}"), &[], &rules)
    .result;
  assert!(out.stdout.contains("mcp saw mcp__x__y"), "{}", out.stdout);
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
  assert!(
    out.stdout.contains("\"hookEventName\": \"PostToolUse\""),
    "{}",
    out.stdout
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
    r#"{"version":2,"spec":"a@t","name":"v2","event":"tool/pre","matcher":"*"}"#,
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
  assert!(
    out.stdout.contains("\"systemMessage\": \"after\""),
    "{}",
    out.stdout
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
  let dispatched = hook.run(
    pre,
    &payload("Bash", r#"{"command":"ls"}"#),
    &[],
    &[&rule, &off],
  );
  assert!(
    dispatched.result.stdout.contains("r saw Bash"),
    "{}",
    dispatched.result.stdout
  );
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
