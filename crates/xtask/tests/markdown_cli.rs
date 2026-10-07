//! `cargo xtask check-markdown-cli` end to end (#444): the real binary on temp
//! roots built from this repository's command tree, allowlist and Markdown.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::Value;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const TREE: &str = "docs/cli/commands.json";
const LIST: &str = "tooling/conventions/markdown-cli.json";

type Result<T> = std::result::Result<T, String>;

fn text<E: std::fmt::Display>(err: E) -> String {
  err.to_string()
}

fn real(path: &str) -> Result<String> {
  std::fs::read_to_string(Path::new(REPO).join(path)).map_err(text)
}

fn real_tree() -> Result<Value> {
  serde_json::from_str(&real(TREE)?).map_err(text)
}

/// The real tree with `namespace`'s placeholder verb renamed to `verb`.
fn ported(namespace: &str, verb: &str) -> Result<Value> {
  let mut tree = real_tree()?;
  let command = tree
    .get_mut("commands")
    .and_then(Value::as_array_mut)
    .and_then(|commands| {
      commands
        .iter_mut()
        .find(|command| command.get("name") == Some(&Value::from(namespace)))
    })
    .ok_or(format!("no namespace {namespace}"))?;
  let verbs = command
    .get_mut("commands")
    .and_then(Value::as_array_mut)
    .ok_or("no verbs")?;
  for child in verbs.iter_mut().filter_map(Value::as_object_mut) {
    if child.get("placeholder") == Some(&Value::Bool(true)) {
      child.insert("placeholder".to_owned(), Value::Bool(false));
      child.insert("name".to_owned(), Value::from(verb));
    }
  }
  Ok(tree)
}

fn write(root: &Path, path: &str, body: &str) -> Result<()> {
  let full = root.join(path);
  std::fs::create_dir_all(full.parent().ok_or("no parent")?).map_err(text)?;
  std::fs::write(full, body).map_err(text)
}

/// The real allowlist's external commands; its allowances name files a temp
/// root lacks.
fn external_only() -> Result<Value> {
  let mut list: Value = serde_json::from_str(&real(LIST)?).map_err(text)?;
  list.as_object_mut().ok_or("not an object")?.remove("allow");
  Ok(list)
}

/// A temp root with `tree`, the real external commands and `files`.
fn root(tree: &Value, files: &[(&str, &str)]) -> Result<tempfile::TempDir> {
  let dir = tempfile::tempdir().map_err(text)?;
  write(dir.path(), TREE, &tree.to_string())?;
  write(dir.path(), LIST, &external_only()?.to_string())?;
  for (path, body) in files {
    write(dir.path(), path, body)?;
  }
  Ok(dir)
}

/// Exit code and stderr of the task on `root`.
fn check(root: &Path) -> Result<(i32, String)> {
  let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
    .args(["check-markdown-cli", "--root"])
    .arg(root)
    .output()
    .map_err(text)?;
  let code = out.status.code().ok_or("killed by a signal")?;
  Ok((code, String::from_utf8(out.stderr).map_err(text)?))
}

fn assert_has(text: &str, parts: &[&str]) {
  for part in parts {
    assert!(text.contains(part), "missing {part:?} in:\n{text}");
  }
}

const SKILL: &str = "plugins/epic-orchestrator/skills/x/SKILL.md";

/// A skill whose line 12 runs `command` in a bash fence.
fn skill_running(command: &str) -> String {
  format!("# X\n\n{}```bash\n{command}\n```\n", "text\n".repeat(8))
}

#[test]
fn an_unknown_verb_names_the_file_line_and_closest_verb() {
  let dir = root(
    &real_tree().unwrap(),
    &[(SKILL, &skill_running("toolu epic planed"))],
  )
  .unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 1, "{stderr}");
  assert_has(
    &stderr,
    &[
      &format!("{SKILL}:12: `toolu epic planed`: unknown command `planed`"),
      "closest: `planned`",
    ],
  );
}

#[test]
fn an_unknown_flag_lists_the_valid_flags() {
  let dir = root(
    &real_tree().unwrap(),
    &[(SKILL, "Run `toolu --jsn epic planned`.\n")],
  )
  .unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 1, "{stderr}");
  assert_has(
    &stderr,
    &[
      "unknown flag `--jsn` on `toolu`",
      "valid: --json, --quiet/-q, --host, --config-dir",
    ],
  );
}

#[test]
fn this_repository_passes_in_under_five_seconds() {
  let started = Instant::now();
  let (code, stderr) = check(Path::new(REPO)).unwrap();
  let elapsed = started.elapsed();
  assert_eq!(code, 0, "{stderr}");
  assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
}

#[test]
fn an_unported_namespace_has_only_its_placeholder_verb() {
  let dir = root(
    &real_tree().unwrap(),
    &[(SKILL, &skill_running("toolu babysit run --pr 1"))],
  )
  .unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 1, "{stderr}");
  assert_has(&stderr, &["unknown command `run` under `toolu babysit`"]);
}

#[test]
fn delivery_flow_runs_toolu_ledger_and_the_bundle_is_a_removed_surface() {
  let execution = "plugins/delivery-flow/skills/delivery-flow/references/execution.md";
  let text = real(execution).unwrap();
  assert!(text.contains("`toolu ledger run <plan_doc> --verify`"));
  let (code, stderr) = check(
    root(&real_tree().unwrap(), &[(execution, &text)])
      .unwrap()
      .path(),
  )
  .unwrap();
  assert_eq!(code, 0, "{stderr}");
  let before = text.replace(
    "`toolu ledger ",
    "`bun \"$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js\" ",
  );
  let (code, stderr) = check(
    root(&real_tree().unwrap(), &[(execution, &before)])
      .unwrap()
      .path(),
  )
  .unwrap();
  assert_eq!(code, 1, "{stderr}");
  assert_has(
    &stderr,
    &[&format!(
      "{execution}:22: `hooks/dist/plan-ledger.js`: a removed surface: `toolu ledger` is ported"
    )],
  );
}

#[test]
fn an_external_command_passes_only_while_it_is_listed() {
  let files = [(
    SKILL,
    skill_running("ast-grep run --pattern 'foo($A)' --lang ts src"),
  )];
  let files: Vec<(&str, &str)> = files
    .iter()
    .map(|(path, text)| (*path, text.as_str()))
    .collect();
  let dir = root(&real_tree().unwrap(), &files).unwrap();
  assert_eq!(check(dir.path()).unwrap().0, 0);
  let mut list = external_only().unwrap();
  list["external"]
    .as_array_mut()
    .unwrap()
    .retain(|name| name != "ast-grep");
  write(dir.path(), LIST, &list.to_string()).unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 1, "{stderr}");
  assert_has(&stderr, &[&format!("{SKILL}:12: `ast-grep`: not `toolu`")]);
}

#[test]
fn renaming_a_verb_without_the_skill_fails_on_the_skill_line() {
  let babysit = "plugins/pr-babysit/skills/babysit/SKILL.md";
  let files = [(babysit, skill_running("toolu babysit tick --json"))];
  let files: Vec<(&str, &str)> = files
    .iter()
    .map(|(path, text)| (*path, text.as_str()))
    .collect();
  let (code, stderr) = check(
    root(&ported("babysit", "tick").unwrap(), &files)
      .unwrap()
      .path(),
  )
  .unwrap();
  assert_eq!(code, 0, "{stderr}");
  let (code, stderr) = check(
    root(&ported("babysit", "step").unwrap(), &files)
      .unwrap()
      .path(),
  )
  .unwrap();
  assert_eq!(code, 1, "{stderr}");
  assert_has(
    &stderr,
    &[
      &format!("{babysit}:12: `toolu babysit tick --json`: unknown command `tick`"),
      "closest: `step`",
    ],
  );
}

#[test]
fn allowances_excuse_their_finding_and_go_stale_without_it() {
  let plugin_list = "tooling/conventions/markdown-cli/epic-orchestrator.json";
  let entry = |file: &str, reason: &str| {
    format!(
      r#"{{"allow": [{{"file": "{file}", "subject": "toolu epic strat", "reason": "{reason}"}}]}}"#
    )
  };
  let dir = root(
    &real_tree().unwrap(),
    &[
      (SKILL, &skill_running("toolu epic strat")),
      (plugin_list, &entry(SKILL, "shows the typo error")),
    ],
  )
  .unwrap();
  assert_eq!(check(dir.path()).unwrap().0, 0);
  write(dir.path(), SKILL, "# fixed\n").unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 1, "{stderr}");
  assert_has(
    &stderr,
    &[&format!(
      "{plugin_list}: stale allowance `toolu epic strat` in {SKILL}"
    )],
  );
  write(dir.path(), plugin_list, &entry(SKILL, "")).unwrap();
  assert_eq!(check(dir.path()).unwrap().0, 2);
  write(
    dir.path(),
    plugin_list,
    &entry("plugins/jev/skills/jev/SKILL.md", "elsewhere"),
  )
  .unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 2);
  assert_has(&stderr, &["outside this allowlist's plugin"]);
}

#[test]
fn a_missing_or_broken_tree_is_a_setup_error() {
  let dir = tempfile::tempdir().unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 2);
  assert_has(
    &stderr,
    &["cannot read docs/cli/commands.json", "cargo xtask docs-cli"],
  );
  write(dir.path(), TREE, "not json").unwrap();
  let (code, stderr) = check(dir.path()).unwrap();
  assert_eq!(code, 2);
  assert_has(&stderr, &["docs/cli/commands.json is not JSON"]);
}
