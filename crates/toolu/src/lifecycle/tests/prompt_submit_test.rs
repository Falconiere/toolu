use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{PromptInput, prompt_submit};

const STRUCTURAL: &str = "Structural pattern: use `ast-grep run --pattern` (not Grep).";
const MISSING: &str =
  "WARN: ast-grep not installed — install via brew/cargo for structural matching.";

#[test]
fn structural_prompts_follow_whether_ast_grep_is_on_path() {
  let dir = tempfile::tempdir().unwrap();
  let with_tool = bin_with(dir.path(), true);
  let found = document(
    &env(dir.path(), &with_tool),
    dir.path(),
    r#"{"prompt":"list all methods on the trait"}"#,
  );
  assert_eq!(context(&found), STRUCTURAL);
  let without = bin_with(dir.path(), false);
  let missing = document(
    &env(dir.path(), &without),
    dir.path(),
    r#"{"prompt":"list all methods on the trait"}"#,
  );
  assert_eq!(context(&missing), MISSING);
}

#[test]
fn a_one_word_verb_blocks_and_trivial_or_empty_prompts_are_silent() {
  let dir = tempfile::tempdir().unwrap();
  let path = bin_with(dir.path(), false);
  let env = env(dir.path(), &path);
  let blocked = document(&env, dir.path(), r#"{"prompt":"fix"}"#);
  assert_eq!(blocked["decision"], "block");
  assert_eq!(
    blocked["reason"],
    "Prompt too vague - specify what file/feature/error needs attention"
  );
  assert!(blocked.get("hookSpecificOutput").is_none());
  assert_eq!(run(&env, dir.path(), r#"{"prompt":"y"}"#).stdout, None);
  assert_eq!(run(&env, dir.path(), r#"{"prompt":""}"#).stdout, None);
  assert_eq!(run(&env, dir.path(), "{}").stdout, None);
}

#[test]
fn a_disabled_hook_prints_nothing() {
  let dir = tempfile::tempdir().unwrap();
  let cfg = dir.path().join("cfg");
  std::fs::create_dir_all(&cfg).unwrap();
  std::fs::write(
    cfg.join("toolu.config.json"),
    r#"{"hooks":{"user-prompt-submit":false}}"#,
  )
  .unwrap();
  let env = env(dir.path(), &bin_with(dir.path(), false));
  let outcome = run(&env, dir.path(), r#"{"prompt":"fix"}"#);
  assert_eq!(outcome.stdout, None);
  assert_eq!(outcome.stderr, None);
}

#[test]
fn a_failing_gate_is_named_unless_the_prompt_is_already_about_it() {
  let dir = tempfile::tempdir().unwrap();
  let state = dir.path().join(".claude").join("tmp");
  std::fs::create_dir_all(&state).unwrap();
  std::fs::write(
    state.join("quality-gate-status.json"),
    r#"{"status":"failing","reason":"lint broke"}"#,
  )
  .unwrap();
  let env = env(dir.path(), &bin_with(dir.path(), false));
  let rename = context(&document(
    &env,
    dir.path(),
    r#"{"prompt":"rename the helper"}"#,
  ));
  assert!(rename.contains("Rename: find all refs"));
  assert!(
    rename.contains("Quality gate failing: lint broke. Prefer fixing before unrelated work.")
  );
  let fixing = context(&document(
    &env,
    dir.path(),
    r#"{"prompt":"fix the parser"}"#,
  ));
  assert!(fixing.contains("Fix in code."));
  assert!(!fixing.contains("Quality gate failing"));
}

#[test]
fn project_context_appends_the_script_stdout() {
  let dir = tempfile::tempdir().unwrap();
  let claude = dir.path().join(".claude");
  std::fs::create_dir_all(&claude).unwrap();
  std::fs::write(claude.join("context.sh"), "printf '%s\\n\\n' \"$PROMPT\"\n").unwrap();
  let bin = dir.path().join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  std::os::unix::fs::symlink("/bin/bash", bin.join("bash")).unwrap();
  let env = env(dir.path(), &bin);
  let text = context(&document(
    &env,
    dir.path(),
    r#"{"prompt":"rename the helper"}"#,
  ));
  assert_eq!(
    text,
    "Rename: find all refs (ast-grep + Grep on configs) before rewriting. | rename the helper"
  );
}

fn bin_with(root: &Path, ast_grep: bool) -> PathBuf {
  let bin = root.join(if ast_grep { "with-sg" } else { "no-sg" });
  std::fs::create_dir_all(&bin).unwrap();
  if ast_grep {
    let sg = bin.join("sg");
    std::fs::write(&sg, "").unwrap();
    let mut perms = std::fs::metadata(&sg).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&sg, perms).unwrap();
  }
  bin
}

fn env(root: &Path, path: &Path) -> Env {
  Env::from_pairs([
    ("HOME", root.join("home").to_str().unwrap()),
    ("PATH", path.to_str().unwrap()),
    ("TOOLU_CONFIG_DIR", root.join("cfg").to_str().unwrap()),
    ("CLAUDE_PROJECT_DIR", root.to_str().unwrap()),
  ])
}

fn run(env: &Env, cwd: &Path, payload: &str) -> toolu_runtime::cli::Outcome {
  let outcome = prompt_submit(&PromptInput { env, cwd, payload });
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  outcome
}

fn document(env: &Env, cwd: &Path, payload: &str) -> Value {
  let outcome = run(env, cwd, payload);
  let stdout = outcome.stdout.as_deref().expect("stdout");
  serde_json::from_str(stdout).unwrap_or_else(|err| panic!("{err}: {stdout}"))
}

fn context(value: &Value) -> String {
  value["hookSpecificOutput"]["additionalContext"]
    .as_str()
    .unwrap()
    .to_owned()
}
