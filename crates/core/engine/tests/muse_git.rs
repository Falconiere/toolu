//! Muse shell gate decisions around real local Git commits and pushes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use toolu_engine::builtins::{POST_TOOL, PRE_TOOL};
use toolu_engine::ledger::io::write_ledger;
use toolu_engine::waiver::Waivers;
use toolu_engine::{DispatchOptions, dispatch_post_tool, dispatch_pre_tool};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;

fn git(cwd: &Path, args: &[&str]) -> std::io::Result<Output> {
  let output = Command::new("git")
    .arg("-C")
    .arg(cwd)
    .args(args)
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .output()?;
  if !output.status.success() {
    return Err(std::io::Error::other(format!("git {args:?}: {output:?}")));
  }
  Ok(output)
}

fn commit(repo: &Path, message: &str) -> std::io::Result<Output> {
  git(
    repo,
    &[
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "commit",
      "-m",
      message,
    ],
  )
}

fn prepare_repository(repo: &Path, remote: &Path) -> Result<(), Box<dyn std::error::Error>> {
  git(repo, &["init", "-b", "main"])?;
  git(remote, &["init", "--bare"])?;
  fs::write(repo.join("main.rs"), "fn main() {}\n")?;
  git(repo, &["add", "main.rs"])?;
  commit(repo, "feat: initial")?;
  let remote = remote.to_str().ok_or("remote path")?;
  git(repo, &["remote", "add", "origin", remote])?;
  git(repo, &["push", "origin", "main"])?;
  git(repo, &["switch", "-c", "feature"])?;
  fs::write(repo.join("main.rs"), "fn main() { println!(\"ok\"); }\n")?;
  git(repo, &["add", "main.rs"])?;
  commit(repo, "feat: change")?;
  Ok(())
}

struct GitSite {
  _dir: tempfile::TempDir,
  repo: PathBuf,
  config: PathBuf,
  settings: PathBuf,
  state: PathBuf,
  ledger: PathBuf,
  env: Env,
}

impl GitSite {
  fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let repo = dir.path().join("repo");
    let remote = dir.path().join("remote.git");
    let config = dir.path().join("config");
    let settings = dir.path().join("settings");
    let state = dir.path().join("state");
    let ledger = dir.path().join("ledger");
    for path in [&repo, &remote, &config, &settings, &state, &ledger] {
      fs::create_dir_all(path)?;
    }
    prepare_repository(&repo, &remote)?;
    fs::write(
      config.join("toolu.config.json"),
      json!({"version":1,"gates":{"commitGate":{"mode":"block"},"pushReview":{"mode":"block"}}})
        .to_string(),
    )?;
    let env = Env::from_pairs([
      ("HOME", dir.path().to_string_lossy().into_owned()),
      ("PATH", std::env::var("PATH")?),
      ("MUSE_PLUGIN_ID", "toolu".to_owned()),
      ("TOOLU_CONFIG_DIR", config.to_string_lossy().into_owned()),
      (
        "TOOLU_SETTINGS_DIR",
        settings.to_string_lossy().into_owned(),
      ),
      ("STATE_DIR", state.to_string_lossy().into_owned()),
      ("LEDGER_DIR", ledger.to_string_lossy().into_owned()),
      ("PUSH_REVIEW_BASE", "main".to_owned()),
    ]);
    Ok(Self {
      _dir: dir,
      repo,
      config,
      settings,
      state,
      ledger,
      env,
    })
  }

  fn options(&self, post: bool) -> DispatchOptions<'_> {
    DispatchOptions {
      env: &self.env,
      cwd: &self.repo,
      lib_dir: &self.config,
      builtins: if post { POST_TOOL } else { PRE_TOOL },
      rules: &[],
      selected_specs: None,
      continue_post_blocks: false,
      split_by_caller: false,
      module_timeout: Duration::from_secs(1),
    }
  }

  fn payload(&self, command: &str, response: Option<String>) -> String {
    let mut payload = json!({
      "hook_event_name":if response.is_some() { "PostToolUse" } else { "PreToolUse" },
      "session_id":"s1", "cwd":self.repo, "tool_name":"bash",
      "tool_input":{"command":command}
    });
    if let Some(response) = response
      && let Some(fields) = payload.as_object_mut()
    {
      fields.insert("tool_response".to_owned(), response.into());
    }
    payload.to_string()
  }
}

#[test]
fn muse_bash_commit_prefix_is_denied_and_plain_echo_is_allowed() {
  let site = GitSite::new().expect("sandbox");
  fs::write(site.settings.join("commit-prefixes.txt"), "feat\nfix\n").expect("prefixes");
  let bad = dispatch_pre_tool(
    &site.payload("git commit -m 'weird: change'", None),
    &site.options(false),
  );
  let output: Value = serde_json::from_str(&bad.result.stdout).expect("commit decision");
  assert_eq!(output["decision"], "block", "{bad:?}");
  assert!(
    output["reason"]
      .as_str()
      .is_some_and(|reason| reason.contains("Unknown Conventional Commits prefix"))
  );
  let echo = dispatch_pre_tool(&site.payload("echo ok", None), &site.options(false));
  assert_eq!(echo.result.exit_code, 0, "{echo:?}");
  assert_eq!(echo.result.stdout, "", "{echo:?}");
}

#[test]
fn muse_push_review_blocks_then_real_push_promotes_a_waiver() {
  let site = GitSite::new().expect("sandbox");
  let command = "git push origin feature";
  let before = dispatch_pre_tool(&site.payload(command, None), &site.options(false));
  let denied: Value = serde_json::from_str(&before.result.stdout).expect("review denial");
  assert_eq!(denied["decision"], "block", "{before:?}");
  assert!(
    denied["reason"]
      .as_str()
      .is_some_and(|reason| reason.contains("Code review required before push"))
  );
  let roots = Roots::new(site.env.clone(), Some(Host::Muse));
  let sha = diff_sha(&site.env, &site.repo, "main").expect("branch diff");
  let waivers = Waivers {
    roots: &roots,
    root: Some(&site.repo),
    slug: "feature",
  };
  assert!(waivers.pend(&sha, "main", "test-approval", SystemTime::now()));
  let pushed = git(&site.repo, &["push", "origin", "feature"]).expect("real push");
  let response = json!({
    "exit_code":pushed.status.code(),
    "output":String::from_utf8_lossy(&pushed.stdout).to_string(),
    "stderr":String::from_utf8_lossy(&pushed.stderr).to_string()
  });
  let post = dispatch_post_tool(
    &site.payload(command, Some(response.to_string())),
    &site.options(true),
  );
  assert_eq!(post.result.exit_code, 0, "{post:?}");
  assert!(site.state.join("feature.waiver.json").is_file());
  let after = dispatch_pre_tool(&site.payload(command, None), &site.options(false));
  let output: Value = serde_json::from_str(&after.result.stdout).unwrap_or(Value::Null);
  assert_ne!(output["decision"], "block", "{after:?}");
}

#[test]
fn muse_push_checks_the_real_code_only_diff_for_docs_sync() {
  let site = GitSite::new().expect("sandbox");
  fs::write(
    site.config.join("toolu.config.json"),
    json!({"version":1,"gates":{"docsSync":{"mode":"block"},"pushReview":{"mode":"block"}}})
      .to_string(),
  )
  .expect("strict docs config");
  let push = site.payload("git push origin feature", None);
  let before = dispatch_pre_tool(&push, &site.options(false));
  let denied: Value = serde_json::from_str(&before.result.stdout).expect("docs denial");
  assert_eq!(denied["decision"], "block", "{before:?}");
  assert!(
    denied["reason"]
      .as_str()
      .is_some_and(|reason| reason.contains("docs-sync"))
  );
  fs::create_dir_all(site.repo.join("docs")).expect("docs directory");
  fs::write(site.repo.join("docs/feature.md"), "# Feature\n").expect("documentation");
  git(&site.repo, &["add", "docs/feature.md"]).expect("stage documentation");
  commit(&site.repo, "docs: describe feature").expect("commit documentation");
  let after = dispatch_pre_tool(&push, &site.options(false));
  let denied: Value = serde_json::from_str(&after.result.stdout).expect("review denial");
  assert!(
    denied["reason"]
      .as_str()
      .is_some_and(|reason| reason.contains("Code review required before push")),
    "{after:?}"
  );
}

#[test]
fn muse_push_reads_a_persisted_plan_ledger_step() {
  let site = GitSite::new().expect("sandbox");
  fs::write(
    site.config.join("toolu.config.json"),
    json!({"version":1,"gates":{"planLedger":{"mode":"block"},"pushReview":{"mode":"block"}}})
      .to_string(),
  )
  .expect("strict ledger config");
  let file = site.ledger.join("feature.json");
  let red = json!({
    "version":1,"plan_doc":"docs/plan.md","summary":{"total":1},
    "steps":[{"id":"compile","title":"Compile the branch","status":"red"}]
  });
  write_ledger(&file, &Ordered::from(&red)).expect("real ledger file");
  let push = site.payload("git push origin feature", None);
  let before = dispatch_pre_tool(&push, &site.options(false));
  let denied: Value = serde_json::from_str(&before.result.stdout).expect("ledger denial");
  assert_eq!(denied["decision"], "block", "{before:?}");
  assert!(
    denied["reason"]
      .as_str()
      .is_some_and(|reason| reason.contains("compile: red"))
  );
  let sha = diff_sha(&site.env, &site.repo, "main").expect("branch diff");
  let green = json!({
    "version":1,"plan_doc":"docs/plan.md","summary":{"total":1},"verified_sha":sha,
    "steps":[{"id":"compile","title":"Compile the branch","status":"green","diff_sha":sha}]
  });
  write_ledger(&file, &Ordered::from(&green)).expect("verified ledger file");
  let after = dispatch_pre_tool(&push, &site.options(false));
  let denied: Value = serde_json::from_str(&after.result.stdout).expect("review denial");
  assert!(
    denied["reason"]
      .as_str()
      .is_some_and(|reason| reason.contains("Code review required before push")),
    "{after:?}"
  );
}
