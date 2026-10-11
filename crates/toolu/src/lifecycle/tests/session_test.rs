use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::{SessionInput, session_start};

const VERSION: &str = "9.9.9";
const EXE: &str = "/usr/local/bin/toolu";

#[test]
fn startup_prints_the_protocol_and_the_native_runtime_line() {
  let (dir, repo, env) = repo_env();
  let outcome = run(&env, &repo, r#"{"source":"startup"}"#, None);
  let value = document(&outcome);
  assert_eq!(outcome.stderr, None);
  assert!(!outcome.stdout.as_deref().unwrap().ends_with('\n'));
  assert!(
    outcome
      .stdout
      .as_deref()
      .unwrap()
      .find("hookSpecificOutput")
      < outcome.stdout.as_deref().unwrap().find("systemMessage")
  );
  assert_eq!(value["hookSpecificOutput"]["hookEventName"], "SessionStart");
  let context = value["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .unwrap();
  assert!(context.contains("Session Protocol — repo"));
  assert!(context.contains("Project: repo"));
  assert!(context.contains("Branch: feature"));
  assert!(context.contains("ast-grep (structural code search)"));
  assert_eq!(
    value["systemMessage"],
    format!("Toolu is on!\ntoolu runtime: native {VERSION} at {EXE}")
  );
  let compact = document(&run(&env, &repo, r#"{"source":"compact"}"#, None));
  assert_eq!(compact["systemMessage"], "Context compacted");
  assert!(
    compact["hookSpecificOutput"]["additionalContext"]
      .as_str()
      .unwrap()
      .contains("Recover memories before continuing")
  );
  let _keep = dir;
}

#[test]
fn a_disabled_hook_keeps_startup_and_resume_and_is_silent_otherwise() {
  let (dir, repo, env) = repo_env();
  let config = dir.path().join("cfg");
  std::fs::create_dir_all(&config).unwrap();
  std::fs::write(
    config.join("toolu.config.json"),
    r#"{"hooks":{"session-start":false}}"#,
  )
  .unwrap();
  let startup = document(&run(&env, &repo, r#"{"source":"startup"}"#, None));
  assert!(startup.get("hookSpecificOutput").is_none());
  assert_eq!(
    startup["systemMessage"],
    format!("Toolu is on!\ntoolu runtime: native {VERSION} at {EXE}")
  );
  let compact = run(&env, &repo, r#"{"source":"compact"}"#, None);
  assert_eq!(compact.stdout, None);
  assert_eq!(compact.stderr, None);
  let advised = document(&run(
    &env,
    &repo,
    r#"{"source":"compact"}"#,
    Some("skew advisory"),
  ));
  assert_eq!(advised["systemMessage"], "skew advisory");
  assert!(advised.get("hookSpecificOutput").is_none());
  let _keep = dir;
}

#[test]
fn codex_startup_removes_the_legacy_statusline_symlink() {
  let dir = tempfile::tempdir().unwrap();
  let codex = dir.path().join("codex");
  let toolu = codex.join("toolu");
  std::fs::create_dir_all(&toolu).unwrap();
  let target = toolu.join("target");
  std::fs::write(&target, "x").unwrap();
  let link = toolu.join("statusline.sh");
  std::os::unix::fs::symlink(&target, &link).unwrap();
  let home = dir.path().join("home");
  let env = Env::from_pairs([
    ("HOME", home.to_str().unwrap()),
    ("CODEX_HOME", codex.to_str().unwrap()),
    ("PATH", "/usr/bin:/bin"),
    ("TOOLU_HOST_OVERRIDE", "codex"),
    ("CLAUDE_PLUGIN_ROOT", plugin_root().to_str().unwrap()),
    ("CLAUDE_PROJECT_DIR", dir.path().to_str().unwrap()),
  ]);
  let outcome = run(&env, dir.path(), r#"{"source":"startup"}"#, None);
  assert!(outcome.stdout.unwrap().contains("Toolu is on!"));
  assert!(std::fs::symlink_metadata(&link).is_err());
  assert!(target.is_file());
}

#[test]
fn without_git_the_branch_and_project_lines_are_omitted() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("bin");
  let work = dir.path().join("work");
  let cfg = dir.path().join("cfg");
  std::fs::create_dir_all(&bin).unwrap();
  std::fs::create_dir_all(&work).unwrap();
  std::fs::create_dir_all(&cfg).unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().join("home").to_str().unwrap()),
    ("PATH", bin.to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", cfg.to_str().unwrap()),
    ("CLAUDE_PROJECT_DIR", work.to_str().unwrap()),
    ("CLAUDE_PLUGIN_ROOT", plugin_root().to_str().unwrap()),
  ]);
  let value = document(&run(&env, &work, r#"{"source":"startup"}"#, None));
  let context = value["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .unwrap();
  assert!(context.contains("Session Protocol — this project"));
  assert!(
    !context
      .lines()
      .any(|line| line.starts_with("Project:") || line.starts_with("Branch:"))
  );
  assert!(
    value["systemMessage"]
      .as_str()
      .unwrap()
      .starts_with("Toolu is on!\n")
  );
}

fn repo_env() -> (tempfile::TempDir, PathBuf, Env) {
  let dir = tempfile::tempdir().unwrap();
  let repo = dir.path().join("repo");
  std::fs::create_dir_all(&repo).unwrap();
  git(&repo, &["init", "-b", "feature"]);
  std::fs::write(repo.join("README"), "x").unwrap();
  git(&repo, &["add", "README"]);
  git(&repo, &["commit", "-m", "init"]);
  let bin = dir.path().join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  std::os::unix::fs::symlink("/usr/bin/git", bin.join("git")).unwrap();
  let home = dir.path().join("home").to_string_lossy().into_owned();
  let cfg = dir.path().join("cfg").to_string_lossy().into_owned();
  let project = repo.to_string_lossy().into_owned();
  let plugin = plugin_root().to_string_lossy().into_owned();
  let path = bin.to_string_lossy().into_owned();
  let env = Env::from_pairs([
    ("HOME", home.as_str()),
    ("PATH", path.as_str()),
    ("TOOLU_CONFIG_DIR", cfg.as_str()),
    ("CLAUDE_PROJECT_DIR", project.as_str()),
    ("CLAUDE_PLUGIN_ROOT", plugin.as_str()),
  ]);
  (dir, repo, env)
}

fn plugin_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/toolu")
}

fn run(env: &Env, cwd: &Path, payload: &str, advisory: Option<&str>) -> Outcome {
  let exe = PathBuf::from(EXE);
  session_start(&SessionInput {
    env,
    cwd,
    plugin_root: None,
    version: VERSION,
    exe: Some(&exe),
    advisory,
    payload,
  })
}

fn document(outcome: &Outcome) -> Value {
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  let stdout = outcome.stdout.as_deref().expect("stdout");
  serde_json::from_str(stdout).unwrap_or_else(|err| panic!("{err}: {stdout}"))
}

fn git(repo: &Path, args: &[&str]) {
  const ENV: &[(&str, &str)] = &[
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "toolu"),
    ("GIT_AUTHOR_EMAIL", "toolu@example.com"),
    ("GIT_COMMITTER_NAME", "toolu"),
    ("GIT_COMMITTER_EMAIL", "toolu@example.com"),
  ];
  let status = Command::new("git")
    .current_dir(repo)
    .envs(ENV.iter().copied())
    .args(args)
    .status()
    .unwrap();
  assert!(status.success(), "{args:?}");
}
