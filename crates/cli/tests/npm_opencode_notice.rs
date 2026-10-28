//! `@toolu/opencode` is a deprecation notice. These tests load
//! `tools/toolu-opencode/npm/index.js` in the real `node` as `OpenCode` V2 does, with a
//! plugin context that records what the plugin asks of it: it logs the notice, and
//! puts it in each session's instructions once, and registers no tool hook.

use std::path::{Path, PathBuf};
use std::process::Command;

fn package() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/toolu-opencode/npm")
}

/// Load the plugin, run its setup, then send two prompts for one session and one
/// for another; print what it logged and asked for as one JSON line.
const DRIVER: &str = r#"
const plugin = (await import(process.argv[1])).default;
const seen = { hooks: [], entries: [], warned: [] };
const hooks = {};
console.warn = (text) => seen.warned.push(String(text));
const ctx = {
  session: {
    hook: async (name, callback) => { seen.hooks.push(`session.${name}`); hooks[name] = callback; },
    instructions: { entry: { put: async (entry) => { seen.entries.push(entry); } } },
  },
  tool: { hook: async (name) => { seen.hooks.push(`tool.${name}`); } },
};
await plugin.setup(ctx);
for (const sessionID of ["s1", "s1", "s2"]) await hooks.prompt({ sessionID, prompt: { text: "hi" } });
process.stdout.write(JSON.stringify({ id: plugin.id, ...seen }));
"#;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn run() -> Res<serde_json::Value> {
  let entry = package().join("index.js");
  let out = Command::new("node")
    .args(["--input-type=module", "-e", DRIVER])
    .arg(format!("file://{}", entry.display()))
    .output()?;
  if !out.status.success() {
    return Err(String::from_utf8_lossy(&out.stderr).into_owned().into());
  }
  Ok(serde_json::from_slice(&out.stdout)?)
}

#[test]
fn the_plugin_reports_the_deprecation_and_runs_no_hooks() {
  let seen = run().unwrap();
  assert_eq!(seen["id"], "toolu.deprecated");
  assert_eq!(seen["hooks"], serde_json::json!(["session.prompt"]));
  let warned = seen["warned"][0].as_str().unwrap();
  assert!(warned.contains("@toolu/opencode is deprecated"), "{warned}");
  assert!(warned.contains("toolu plugins install --host opencode"));
  let sessions: Vec<&str> = seen["entries"]
    .as_array()
    .unwrap()
    .iter()
    .map(|entry| entry["sessionID"].as_str().unwrap())
    .collect();
  assert_eq!(sessions, ["s1", "s2"]);
  assert_eq!(seen["entries"][0]["key"], "toolu.deprecated");
}

#[test]
fn the_manifest_ships_only_the_notice() {
  let text = std::fs::read_to_string(package().join("package.json")).unwrap();
  let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
  assert_eq!(manifest["name"], "@toolu/opencode");
  assert_eq!(manifest["files"], serde_json::json!(["index.js"]));
  assert!(manifest.get("dependencies").is_none(), "{manifest}");
  assert!(manifest.get("scripts").is_none(), "{manifest}");
  assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
}
