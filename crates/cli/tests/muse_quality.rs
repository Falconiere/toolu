//! A real Muse edit through the installed Python quality hook and CLI.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::{RegistryEvent, event_dir};

fn run(
  args: &[&str],
  cwd: &Path,
  env: &[(String, String)],
  input: &str,
) -> std::io::Result<Output> {
  let mut child = Command::new(env!("CARGO_BIN_EXE_toolu"))
    .args(args)
    .current_dir(cwd)
    .env_clear()
    .envs(env.iter().map(|(key, value)| (key, value)))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  child
    .stdin
    .take()
    .ok_or_else(|| std::io::Error::other("toolu stdin unavailable"))?
    .write_all(input.as_bytes())?;
  child.wait_with_output()
}

fn register_python(
  project: &Path,
  env: &[(String, String)],
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync + 'static>> {
  let plugin = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins/python-quality")
    .canonicalize()?;
  let plugin = plugin.to_str().ok_or("plugin path is not UTF-8")?;
  let registered = run(
    &[
      "python-quality",
      "hook",
      "register",
      "--event",
      "SessionStart",
      "--plugin-root",
      plugin,
    ],
    project,
    env,
    "{}",
  )?;
  if !registered.status.success() || !registered.stdout.is_empty() {
    return Err(format!("registration failed: {registered:?}").into());
  }
  let roots = Roots::new(Env::from_pairs(env.to_vec()), Some(Host::Muse));
  Ok(event_dir(&roots, RegistryEvent::ToolPost).join("python-quality@toolu__python-quality.json"))
}

struct MusePythonSite {
  _dir: tempfile::TempDir,
  project: PathBuf,
  source: PathBuf,
  manifest: PathBuf,
  env: Vec<(String, String)>,
}

impl MusePythonSite {
  fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let dir = tempfile::tempdir()?;
    let project = dir.path().join("project");
    let config = dir.path().join("config");
    fs::create_dir_all(&project)?;
    fs::create_dir_all(&config)?;
    fs::write(
      project.join("pyproject.toml"),
      "[project]\nname = \"muse-case\"\nversion = \"0.1.0\"\n",
    )?;
    let source = project.join("sample.py");
    fs::write(&source, "try:\n    pass\nexcept:\n    pass\n")?;
    let git = Command::new("git")
      .arg("init")
      .arg("-b")
      .arg("main")
      .arg(&project)
      .output()?;
    if !git.status.success() {
      return Err(format!("git init failed: {git:?}").into());
    }
    let mut env = vec![
      ("HOME".to_owned(), dir.path().display().to_string()),
      ("PATH".to_owned(), std::env::var("PATH")?),
      ("MUSE_PLUGIN_ID".to_owned(), "toolu".to_owned()),
      ("TOOLU_CONFIG_DIR".to_owned(), config.display().to_string()),
    ];
    if let Ok(tmp) = std::env::var("TMPDIR") {
      env.push(("TMPDIR".to_owned(), tmp));
    }
    let manifest = register_python(&project, &env)?;
    Ok(Self {
      _dir: dir,
      project,
      source,
      manifest,
      env,
    })
  }

  fn set_matcher(
    &self,
    name: &str,
  ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let mut rule: Value = serde_json::from_slice(&fs::read(&self.manifest)?)?;
    let fields = rule.as_object_mut().ok_or("manifest is not an object")?;
    fields.insert("matcher".to_owned(), name.into());
    fs::write(&self.manifest, rule.to_string())?;
    Ok(())
  }

  fn post(&self, name: &str, path: &Path) -> std::io::Result<Output> {
    let input = if name == "write_file" {
      json!({"path":path,"content":fs::read_to_string(path)?})
    } else {
      json!({"path":path,"find":"old","replace":"new"})
    };
    let payload = json!({
      "hook_event_name":"PostToolUse","session_id":"s1","cwd":self.project,
      "tool_name":name,"tool_input":input,
      "tool_response":{"success":true}
    });
    run(
      &["hook", "post-tools", "--event", "PostToolUse"],
      &self.project,
      &self.env,
      &payload.to_string(),
    )
  }

  fn gate_file(&self) -> Option<PathBuf> {
    Roots::new(Env::from_pairs(self.env.clone()), Some(Host::Muse))
      .project_state_root(None, Some(&self.project))
      .map(|root| root.join("quality-gate-status.json"))
  }
}

#[test]
fn muse_edit_file_records_a_real_python_quality_violation() {
  let site = MusePythonSite::new().expect("real Python site");
  site.set_matcher("Write").expect("nonmatching rule");
  let skipped = site
    .post("edit_file", &site.source)
    .expect("nonmatching hook");
  assert!(skipped.status.success(), "{skipped:?}");
  assert!(skipped.stdout.is_empty(), "{skipped:?}");
  site.set_matcher("Edit").expect("matching rule");
  let post = site.post("edit_file", &site.source).expect("matching hook");
  assert!(post.status.success(), "{post:?}");
  let decision: Value = serde_json::from_slice(&post.stdout).expect("Muse post decision");
  assert!(
    decision["systemMessage"]
      .as_str()
      .is_some_and(|message| message.contains("Forbidden suppression")),
    "{post:?}"
  );
  let fresh = site.project.join("fresh.py");
  fs::write(&fresh, "try:\n    pass\nexcept:\n    pass\n").expect("new Python file");
  for path in [&fresh, &site.source] {
    let wrote = site.post("write_file", path).expect("Muse write hook");
    assert!(wrote.status.success(), "{wrote:?}");
    let result: Value = serde_json::from_slice(&wrote.stdout).expect("quality advice");
    assert!(
      result["systemMessage"]
        .as_str()
        .is_some_and(|message| message.contains("Forbidden suppression")),
      "{wrote:?}"
    );
  }
  let state =
    fs::read_to_string(site.gate_file().expect("project state")).expect("quality gate state");
  assert!(state.contains("\"status\": \"failing\""), "{state}");
  assert!(state.contains("fresh.py"), "{state}");
}
