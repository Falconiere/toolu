//! The Bun bridge (AC-7, AC-8): consecutive `.js` modules share one Bun process,
//! a module that hangs or exits the process fails alone, and without Bun a
//! pre-tool `.js` module fails closed while post-tool advises once a session.

#[path = "helpers/bins.rs"]
mod bins;
#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/modules.rs"]
mod modules;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::time::Duration;

use hook::Hook;
use toolu_engine::Phase;
use toolu_engine::trace::{Skip, StepStatus};
use toolu_protocol::host::Host;

const BASH: &str = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"session_id":"s1"}"#;

fn advise(text: &str) -> String {
  format!(r#"return {{ kind: "advisory", message: "{text}" }};"#)
}

/// A hook whose `bun` is a wrapper that logs each spawn to `spawns` and runs the real Bun.
fn logged() -> sandbox::Res<Hook> {
  let mut hook = Hook::new(Host::Claude)?;
  let real = bins::which("bun")?;
  let body = format!(
    "echo spawn >> {}\nexec {real} \"$@\"",
    hook.sb.text("spawns")
  );
  bins::script(&hook.sb.path("bin/bun"), &body)?;
  let path = std::env::var("PATH").map_err(|err| err.to_string())?;
  hook
    .extra
    .push(("PATH".to_owned(), format!("{}:{path}", hook.sb.text("bin"))));
  Ok(hook)
}

fn spawns(hook: &Hook) -> usize {
  std::fs::read_to_string(hook.sb.path("spawns")).map_or(0, |log| log.lines().count())
}

#[test]
fn consecutive_modules_share_one_bun_and_a_deny_stops_the_batch() {
  let hook = logged().unwrap();
  let pre = Phase::Pre;
  modules::js(&hook, pre, "a@t", "one", &advise("one")).unwrap();
  modules::js(
    &hook,
    pre,
    "b@t",
    "two",
    r#"return { kind: "deny", reason: "two denies" };"#,
  )
  .unwrap();
  let marker = hook.sb.text("three-ran");
  modules::js(
    &hook,
    pre,
    "c@t",
    "three",
    &format!("await Bun.write({marker:?}, \"x\"); return {{ kind: \"allow\" }};"),
  )
  .unwrap();
  modules::manifest(&hook, pre, "b@t", "rule", "Write").unwrap();
  modules::file(&hook, pre, "ab@t__off.js", "export default 42;").unwrap();
  modules::install(&hook, &["a@t", "b@t", "c@t"]).unwrap();
  let out = hook.run(pre, BASH, &[], &[]).result;
  assert_eq!(
    out.stdout,
    "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"two denies\"}}\n"
  );
  assert_eq!(
    spawns(&hook),
    1,
    "an inactive module and a non-matching manifest do not split the batch"
  );
  assert!(
    !hook.sb.path("three-ran").exists(),
    "nothing runs after the deny"
  );
}

#[test]
fn an_executable_between_modules_splits_the_batch_and_stderr_passes_through() {
  let hook = logged().unwrap();
  let pre = Phase::Pre;
  modules::js(
    &hook,
    pre,
    "a@t",
    "one",
    r#"console.error("from bun"); return { kind: "advisory", message: "one" };"#,
  )
  .unwrap();
  hook
    .sh(
      pre,
      "b@t__mid.sh",
      r#"printf '%s\n' '{"systemMessage":"mid"}'"#,
    )
    .unwrap();
  modules::js(&hook, pre, "c@t", "three", &advise("three")).unwrap();
  let out = hook.run(pre, BASH, &[], &[]).result;
  let doc: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
  assert_eq!(
    doc["hookSpecificOutput"]["additionalContext"],
    "one\n\nthree"
  );
  assert_eq!(doc["systemMessage"], "mid");
  assert_eq!(out.stderr, "from bun\n");
  assert_eq!(spawns(&hook), 2);
}

#[test]
fn a_module_that_hangs_or_exits_fails_alone() {
  let mut hook = logged().unwrap();
  hook.timeout = Duration::from_secs(1);
  let pre = Phase::Pre;
  modules::js(&hook, pre, "a@t", "one", &advise("one")).unwrap();
  modules::js(&hook, pre, "b@t", "hang", "while (true) {}").unwrap();
  modules::js(&hook, pre, "c@t", "exits", "process.exit(3);").unwrap();
  modules::js(&hook, pre, "d@t", "four", &advise("four")).unwrap();
  let out = hook.run(pre, BASH, &[], &[]).result;
  assert_eq!(
    out.stderr,
    "toolu-registry: module b@t__hang.js failed: timed out after 4000 ms; output skipped\n\
     toolu-registry: module c@t__exits.js failed: bridge exited 3; output skipped\n"
  );
  let doc: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
  assert_eq!(
    doc["hookSpecificOutput"]["additionalContext"],
    "one\n\nfour"
  );
  assert_eq!(spawns(&hook), 3);
}

#[test]
fn a_project_bun_is_not_the_bridges_and_nothing_follows_a_stop() {
  let hook = logged().unwrap();
  bins::script(
    &hook.sb.path("project/node_modules/.bin/bun"),
    "echo local >> ../spawns\nexit 1",
  )
  .unwrap();
  modules::js(&hook, Phase::Post, "x@t", "m", &advise("from js")).unwrap();
  let edit = r#"{"tool_name":"Bash","tool_input":{"command":"ls"},"tool_response":{}}"#;
  let out = hook.run(Phase::Post, edit, &[], &[]).result;
  let advice = "{\n  \"hookSpecificOutput\": {\n    \"hookEventName\": \"PostToolUse\",\n    \"additionalContext\": \"from js\"\n  }\n}\n";
  assert_eq!(
    (out.stdout.as_str(), out.stderr.as_str()),
    (advice, ""),
    "path {:?} spawns {:?}",
    hook.env().get("PATH"),
    std::fs::read_to_string(hook.sb.path("spawns")).ok()
  );
  assert_eq!(
    std::fs::read_to_string(hook.sb.path("spawns")).unwrap(),
    "spawn\n"
  );
  modules::js(
    &hook,
    Phase::Pre,
    "a@t",
    "deny",
    r#"return { kind: "deny", reason: "no" };"#,
  )
  .unwrap();
  modules::file(&hook, Phase::Pre, "b@t__bad.json", "{").unwrap();
  let out = hook.run(Phase::Pre, BASH, &[], &[]).result;
  assert_eq!(out.stderr, "", "the walk stopped before the bad manifest");
}

/// A hook with no Bun anywhere: an empty `PATH` and a home without `.bun`.
fn bunless(host: Host) -> sandbox::Res<Hook> {
  let mut hook = Hook::new(host)?;
  hook.extra.push(("PATH".to_owned(), hook.sb.text("empty")));
  Ok(hook)
}

#[test]
fn without_bun_a_pre_tool_module_fails_closed() {
  let hook = bunless(Host::Claude).unwrap();
  modules::js(&hook, Phase::Pre, "x@t", "m", &advise("never")).unwrap();
  let dispatched = hook.run(Phase::Pre, BASH, &[], &[]);
  let doc: serde_json::Value = serde_json::from_str(&dispatched.result.stdout).unwrap();
  assert_eq!(doc["hookSpecificOutput"]["permissionDecision"], "deny");
  assert_eq!(
    doc["hookSpecificOutput"]["permissionDecisionReason"],
    "toolu-registry: module x@t__m.js needs Bun 1.4.x, which was not found (checked TOOLU_BUN, PATH and ~/.bun/bin/bun); install it from https://bun.sh or remove the module"
  );
  assert_eq!(
    dispatched.trace.last().unwrap().status,
    StepStatus::Skipped(Skip::NoBun)
  );
}

#[test]
fn without_bun_a_post_tool_session_is_told_once() {
  let hook = bunless(Host::Claude).unwrap();
  modules::js(&hook, Phase::Post, "x@t", "m", &advise("never")).unwrap();
  modules::js(&hook, Phase::Post, "y@t", "n", &advise("never")).unwrap();
  let post = |session: &str| {
    let payload = format!(
      r#"{{"tool_name":"Edit","tool_input":{{"file_path":"{}","old_string":"a","new_string":"b"}}{session}}}"#,
      hook.sb.text("project/a.ts")
    );
    hook.run(Phase::Post, &payload, &[], &[]).result.stdout
  };
  let first = post(r#","session_id":"s1""#);
  let told = format!(
    "{{\n  \"hookSpecificOutput\": {{\n    \"hookEventName\": \"PostToolUse\",\n    \"additionalContext\": {}\n  }}\n}}\n",
    serde_json::Value::from(
      "toolu-registry: 2 registry module(s) did not run because Bun was not found: x@t__m.js, y@t__n.js. Install Bun 1.4.x from https://bun.sh."
    )
  );
  assert_eq!(first, told);
  assert_eq!(post(r#","session_id":"s1""#), "", "once per session");
  assert_eq!(
    post(""),
    told,
    "no session id counts as `unknown`, a session of its own"
  );
  assert_eq!(post(""), "");
  let markers = hook.sb.path("project/.claude/tmp/registry-bridge");
  std::fs::remove_dir_all(&markers).unwrap();
  std::fs::write(&markers, "in the way").unwrap();
  assert_eq!(post(r#","session_id":"s2""#), told);
  assert_eq!(
    post(r#","session_id":"s2""#),
    told,
    "an unwritable marker repeats the advisory"
  );
}
