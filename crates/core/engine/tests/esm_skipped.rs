//! `.js` registry modules never run (#440): no Bun is looked up or spawned, a
//! `.js` module no longer shadows its spec's `.sh`, and nothing is printed per
//! call; `toolu hook session-start` names them instead.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/modules.rs"]
mod modules;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use hook::Hook;
use toolu_engine::Phase;
use toolu_engine::registry::{esm_advisory, esm_modules};
use toolu_engine::trace::{Skip, StepKind, StepStatus};
use toolu_protocol::host::Host;

const BASH: &str = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"session_id":"s1"}"#;

/// Write an executable `#!/bin/sh` script.
fn script(path: &std::path::Path, body: &str) -> sandbox::Res<()> {
  use std::os::unix::fs::PermissionsExt as _;
  let dir = path.parent().ok_or("no parent")?;
  std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
  std::fs::write(path, format!("#!/bin/sh\n{body}\n")).map_err(|err| err.to_string())?;
  let mode = std::fs::Permissions::from_mode(0o755);
  std::fs::set_permissions(path, mode).map_err(|err| err.to_string())
}

/// A hook whose `PATH`, `TOOLU_BUN` and `~/.bun/bin` hold a `bun` that records any spawn.
fn baited() -> sandbox::Res<Hook> {
  let mut hook = Hook::new(Host::Claude)?;
  let body = format!("echo spawn >> {}\nexit 0", hook.sb.text("spawns"));
  script(&hook.sb.path("bin/bun"), &body)?;
  script(&hook.sb.path("home/.bun/bin/bun"), &body)?;
  let path = std::env::var("PATH").map_err(|err| err.to_string())?;
  hook
    .extra
    .push(("PATH".to_owned(), format!("{}:{path}", hook.sb.text("bin"))));
  hook
    .extra
    .push(("TOOLU_BUN".to_owned(), hook.sb.text("bin/bun")));
  Ok(hook)
}

#[test]
fn a_js_deny_never_runs_and_its_specs_sh_does() {
  let hook = baited().unwrap();
  modules::js(
    &hook,
    Phase::Pre,
    "x@t",
    "deny",
    r#"return { kind: "deny", reason: "js denies" };"#,
  )
  .unwrap();
  hook
    .sh(
      Phase::Pre,
      "x@t__ok.sh",
      "printf '%s\\n' '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"additionalContext\":\"sh ran\"}}'",
    )
    .unwrap();
  modules::install(&hook, &["x@t"]).unwrap();
  let dispatched = hook.run(Phase::Pre, BASH, &[], &[]);
  assert!(!dispatched.result.stdout.contains("deny"), "{dispatched:?}");
  assert!(
    dispatched.result.stdout.contains("sh ran"),
    "{dispatched:?}"
  );
  assert_eq!(dispatched.result.stderr, "");
  assert!(!hook.sb.path("spawns").exists(), "a bun was spawned");
  let js = dispatched
    .trace
    .iter()
    .find(|step| step.module == "x@t__deny.js")
    .unwrap();
  assert_eq!(
    (js.kind, js.status.clone()),
    (StepKind::Esm, StepStatus::Skipped(Skip::Unsupported))
  );
}

#[test]
fn a_post_tool_js_block_never_runs() {
  let hook = baited().unwrap();
  modules::js(
    &hook,
    Phase::Post,
    "x@t",
    "block",
    r#"return { kind: "post_block", reason: "js blocks" };"#,
  )
  .unwrap();
  modules::install(&hook, &["x@t"]).unwrap();
  let edit = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{}}"#;
  let out = hook.run(Phase::Post, edit, &[], &[]).result;
  assert_eq!((out.stdout.as_str(), out.stderr.as_str()), ("", ""));
  assert!(!hook.sb.path("spawns").exists(), "a bun was spawned");
}

#[test]
fn esm_modules_names_each_js_file_by_directory() {
  let hook = Hook::new(Host::Claude).unwrap();
  modules::js(
    &hook,
    Phase::Pre,
    "x@t",
    "deny",
    "return { kind: \"allow\" };",
  )
  .unwrap();
  modules::js(
    &hook,
    Phase::Post,
    "y@t",
    "late",
    "return { kind: \"allow\" };",
  )
  .unwrap();
  hook.sh(Phase::Pre, "z@t__ok.sh", "exit 0").unwrap();
  modules::manifest(&hook, Phase::Pre, "z@t", "rule", "Bash").unwrap();
  modules::file(&hook, Phase::Pre, "nospec.js", "export default 1;").unwrap();
  let found = esm_modules(&hook.config_root());
  assert_eq!(
    found,
    ["pre-tools.d/x@t__deny.js", "post-tools.d/y@t__late.js"]
  );
  assert_eq!(
    esm_advisory(&found[0]),
    "toolu-registry: pre-tools.d/x@t__deny.js is a JavaScript module, which toolu no longer \
     runs; publish it as an executable x@t__deny.sh (docs/registry.md#executable-modules)"
  );
  assert_eq!(esm_modules(&hook.sb.path("absent")), Vec::<String>::new());
}
