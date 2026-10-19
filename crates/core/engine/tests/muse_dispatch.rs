//! Real Muse tool payloads through the native engine and its built-in gates.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_engine::builtins::PRE_TOOL;
use toolu_engine::{DispatchOptions, Dispatched, dispatch_pre_tool};
use toolu_runtime::env::Env;

struct MuseSite {
  _dir: tempfile::TempDir,
  project: PathBuf,
  plugin: PathBuf,
  settings: PathBuf,
  env: Env,
}

impl MuseSite {
  fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let home = dir.path().join("home");
    let project = dir.path().join("project");
    let plugin = dir.path().join("plugin");
    let config = home.join(".config/muse");
    let settings = dir.path().join("settings");
    for path in [&home, &project, &plugin, &config, &settings] {
      fs::create_dir_all(path)?;
    }
    fs::write(
      config.join("toolu.config.json"),
      json!({"version":1,"gates":{"bashCommands":{"mode":"block"},"protectedFiles":{"mode":"block"},"mcpBlocker":{"mode":"block"}}}).to_string(),
    )
    ?;
    let env = Env::from_pairs([
      ("HOME", home.to_string_lossy().into_owned()),
      ("PATH", std::env::var("PATH")?),
      ("MUSE_PLUGIN_ID", "toolu".to_owned()),
      ("TOOLU_CONFIG_DIR", config.to_string_lossy().into_owned()),
      (
        "TOOLU_SETTINGS_DIR",
        settings.to_string_lossy().into_owned(),
      ),
    ]);
    Ok(Self {
      _dir: dir,
      project,
      plugin,
      settings,
      env,
    })
  }

  fn options(&self) -> DispatchOptions<'_> {
    DispatchOptions {
      env: &self.env,
      cwd: &self.project,
      lib_dir: &self.plugin,
      builtins: PRE_TOOL,
      rules: &[],
      selected_specs: None,
      continue_post_blocks: false,
      split_by_caller: false,
      module_timeout: Duration::from_secs(1),
    }
  }

  fn payload(&self, name: &str, input: &Value) -> String {
    json!({
      "hook_event_name":"PreToolUse", "session_id":"s1", "cwd":self.project,
      "tool_name":name, "tool_input":input
    })
    .to_string()
  }

  fn pre(&self, name: &str, input: &Value) -> Dispatched {
    dispatch_pre_tool(&self.payload(name, input), &self.options())
  }
}

fn block_reason(got: &Dispatched) -> Result<String, Box<dyn std::error::Error>> {
  if got.result.exit_code != 0 || !got.result.stderr.is_empty() {
    return Err(format!("unexpected Muse result: {got:?}").into());
  }
  let output: Value = serde_json::from_str(&got.result.stdout)?;
  if output.get("decision").and_then(Value::as_str) != Some("block") {
    return Err(format!("expected Muse block: {got:?}").into());
  }
  Ok(
    output
      .get("reason")
      .and_then(Value::as_str)
      .ok_or("block reason missing")?
      .to_owned(),
  )
}

#[test]
fn raw_muse_bash_reaches_the_real_denylist_gate() {
  let site = MuseSite::new().expect("sandbox");
  fs::write(site.settings.join("bash-denylist.txt"), "node -e\n").expect("denylist");
  let got = site.pre("bash", &json!({"command":"node -e 1"}));
  assert!(
    block_reason(&got)
      .expect("block")
      .contains("Command blocked by deny rule: node -e")
  );
  assert_eq!(got.trace[0].module, "bash-commands");
}

#[test]
fn raw_muse_edit_and_write_use_path_for_protected_files() {
  let site = MuseSite::new().expect("sandbox");
  fs::write(site.settings.join("protected-files.txt"), ".env\n").expect("protected files");
  let path = site.project.join(".env");
  for (name, input) in [
    (
      "edit_file",
      json!({"path":path,"find":"old","replace":"new"}),
    ),
    ("write_file", json!({"path":path,"content":"secret"})),
  ] {
    let got = site.pre(name, &input);
    assert!(
      block_reason(&got)
        .expect("block")
        .contains("protected path"),
      "{name}: {got:?}"
    );
    assert!(
      got
        .trace
        .iter()
        .any(|step| step.module == "protected-files")
    );
  }
}

#[test]
fn raw_muse_source_edit_reaches_code_edit_advice() {
  let site = MuseSite::new().expect("sandbox");
  fs::write(
    site.settings.join("code-edit-rules.json"),
    json!({"rules":[{"match":"src/*.rs","docs":["read docs/rust-quality-bar.md"]}]}).to_string(),
  )
  .expect("code edit rules");
  let path = site.project.join("src/main.rs");
  let got = site.pre(
    "edit_file",
    &json!({"path":path,"find":"old","replace":"new"}),
  );
  assert_eq!(got.result.exit_code, 0, "{got:?}");
  assert_eq!(got.result.stderr, "", "{got:?}");
  let output: Value = serde_json::from_str(&got.result.stdout).expect("Muse advisory");
  assert!(
    output["systemMessage"]
      .as_str()
      .is_some_and(|message| message.contains("File: src/main.rs"))
  );
  assert_eq!(got.trace.len(), PRE_TOOL.len());
}

#[test]
fn raw_muse_mcp_name_reaches_the_dispatch_blocker() {
  let site = MuseSite::new().expect("sandbox");
  fs::write(
    site.settings.join("mcp-blocklist.txt"),
    "plugin_toolu_probe_540_probe_mcp\n",
  )
  .expect("mcp blocklist");
  let got = site.pre(
    "mcp__plugin_toolu_probe_540_probe_mcp__probe_echo",
    &json!({"text":"hello"}),
  );
  assert!(block_reason(&got).expect("block").contains("MCP server"));
  assert!(got.trace.iter().any(|step| step.module == "mcp-blocker"));
}

#[test]
fn raw_muse_internal_tool_passes_through_the_walk() {
  let site = MuseSite::new().expect("sandbox");
  let got = site.pre("submit_reminder_decision", &json!({"decision":"ignore"}));
  assert_eq!(got.result.exit_code, 0, "{got:?}");
  assert_eq!(got.result.stdout, "", "{got:?}");
  assert_eq!(got.result.stderr, "", "{got:?}");
  assert_eq!(got.trace.len(), PRE_TOOL.len());
}
