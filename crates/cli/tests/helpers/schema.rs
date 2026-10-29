//! `toolu commands --json` validates against `toolu commands --schema`, an insta
//! snapshot pins its text, and every other `--json` document validates against
//! its `$defs` entry (AC-4, AC-5).

use serde_json::{Value, json};

use crate::cli::{Res, namespaces, one_document, stderr, stdout, toolu};

/// The schema the binary prints.
fn schema() -> Res<Value> {
  one_document(&toolu(&["commands", "--schema"])?)
}

/// The errors of `document` against `$defs/<def>`, or against the root without one.
pub(crate) fn violations(document: &Value, def: Option<&str>) -> Res<Vec<String>> {
  let schema = schema()?;
  let schema = match def {
    Some(def) => json!({
      "$schema": schema.get("$schema"),
      "$defs": schema.get("$defs"),
      "$ref": format!("#/$defs/{def}"),
    }),
    None => schema,
  };
  let validator = jsonschema::validator_for(&schema).map_err(|err| err.to_string())?;
  Ok(
    validator
      .iter_errors(document)
      .map(|err| err.to_string())
      .collect(),
  )
}

#[test]
fn the_command_tree_validates_against_its_schema() {
  let tree = one_document(&toolu(&["commands", "--json"]).unwrap()).unwrap();
  assert_eq!(violations(&tree, None).unwrap(), Vec::<String>::new());
}

#[test]
fn a_tree_missing_a_required_key_fails_the_schema() {
  let mut tree = one_document(&toolu(&["commands", "--json"]).unwrap()).unwrap();
  tree.as_object_mut().unwrap().remove("hookProtocol");
  assert_ne!(violations(&tree, None).unwrap(), Vec::<String>::new());
  let mut extra = one_document(&toolu(&["commands", "--json"]).unwrap()).unwrap();
  extra["commands"][0]["unexpected"] = json!(true);
  assert_ne!(violations(&extra, None).unwrap(), Vec::<String>::new());
}

#[test]
fn a_snapshot_pins_the_command_tree() {
  let text = stdout(&toolu(&["commands", "--json"]).unwrap()).unwrap();
  let mut settings = insta::Settings::clone_current();
  settings.set_snapshot_path("../fixtures");
  settings.set_prepend_module_to_snapshot(false);
  settings.set_omit_expression(true);
  settings.bind(|| insta::assert_snapshot!("commands_json", text));
}

#[test]
fn every_other_json_document_validates_against_its_definition() {
  for (args, def) in [
    (&["--json", "epik", "start"][..], "error"),
    (&["--json", "--version"], "version"),
    (&["--json", "--help"], "help"),
    (&["--json", "epic", "--help"], "help"),
    (&["epic", "queue", "--json"], "epicQueue"),
    (&["--json", "jev", "noul"], "error"),
    (&["--json", "brainstorm"], "guide"),
  ] {
    let document = one_document(&toolu(args).unwrap()).unwrap();
    assert_eq!(
      violations(&document, Some(def)).unwrap(),
      Vec::<String>::new(),
      "{args:?}"
    );
  }
  assert_eq!(
    violations(&json!({}), Some("empty")).unwrap(),
    Vec::<String>::new()
  );
  assert_ne!(
    violations(&json!({ "error": { "code": 64 } }), Some("error")).unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn epic_status_json_validates_with_an_isolated_resource_home() {
  let home = tempfile::tempdir().unwrap();
  let output = assert_cmd::Command::new(crate::cli::TOOLU)
    .args(["epic", "status", "--json"])
    .env("TOOLU_RESOURCE_HOME", home.path())
    .output()
    .unwrap();
  let document = one_document(&output).unwrap();
  assert_eq!(document["engine"], "down");
  assert_eq!(
    violations(&document, Some("epicStatus")).unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn a_wait_judgment_validates_and_an_unknown_state_does_not() {
  let mut judgment = json!({"kind": "blocked", "key": "a", "epic": "one", "note": "blocked",
    "seq": 1, "state": "open", "age_ms": 0, "pending": "Proceed? (y/n)"});
  assert_eq!(
    violations(&judgment, Some("epicJudgment")).unwrap(),
    Vec::<String>::new()
  );
  judgment["state"] = json!("closed");
  assert_ne!(
    violations(&judgment, Some("epicJudgment")).unwrap(),
    Vec::<String>::new()
  );
}

#[test]
fn every_namespace_s_json_run_is_its_own_valid_document() {
  for name in namespaces().unwrap() {
    let (args, def) = match name.as_str() {
      "hook" | "commands" | "completions" | "doctor" | "ledger" | "debug" | "config" | "status"
      | "setup" | "opencode" | "jev" | "review" | "code-review" | "docs" | "ast-grep"
      | "statusline" | "babysit" | "epic" | "sandbox" | "git-better" | "plugins" => continue,
      "brainstorm" | "delivery-flow" | "ts-quality" | "rust-quality" | "python-quality" => {
        (vec!["--json", name.as_str()], "guide")
      }
      _ => (vec!["--json", name.as_str(), "planned"], "planned"),
    };
    let output = toolu(&args).unwrap();
    assert_eq!(output.status.code(), Some(0), "{name}");
    assert_eq!(stderr(&output).unwrap(), "", "{name}");
    let document = one_document(&output).unwrap();
    assert_eq!(
      violations(&document, Some(def)).unwrap(),
      Vec::<String>::new(),
      "{name}"
    );
    assert_eq!(document["namespace"], name.as_str());
  }
}

/// A `claude` that lists no plugin and accepts every other command, in `<home>/bin`.
fn stub_claude(home: &std::path::Path) -> Res<()> {
  use std::os::unix::fs::PermissionsExt as _;
  let bin = home.join("bin");
  std::fs::create_dir_all(&bin)?;
  let path = bin.join("claude");
  std::fs::write(
    &path,
    "#!/bin/sh\ncase \"$*\" in\n*--json) echo '[]' ;;\nesac\nexit 0\n",
  )?;
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
  Ok(())
}

/// `toolu --json plugins …` in a fresh `HOME` with only the stub `claude` on `PATH`.
fn plugins_json(home: &std::path::Path, args: &[&str]) -> Res<Value> {
  let mut command = assert_cmd::Command::new(crate::cli::TOOLU);
  command
    .args(["--json", "plugins"])
    .args(args)
    .current_dir(home)
    .env_clear()
    .env("HOME", home)
    .env(
      "PATH",
      format!("{}:/usr/bin:/bin", home.join("bin").display()),
    );
  // Coverage runs keep their profile path, so no `.profraw` lands in the cwd.
  if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  let output = command.output()?;
  one_document(&output)
}

#[test]
fn the_plugins_json_documents_validate() {
  let home = tempfile::tempdir().unwrap();
  stub_claude(home.path()).unwrap();
  let install = plugins_json(
    home.path(),
    &["install", "--dry-run", "--host", "claude", "jev"],
  )
  .unwrap();
  assert_eq!(
    violations(&install, Some("pluginsInstall")).unwrap(),
    Vec::<String>::new()
  );
  assert_eq!(install["hosts"][0]["steps"][0]["outcome"], "skipped");
  let list = plugins_json(home.path(), &["list", "--host", "claude"]).unwrap();
  assert_eq!(
    violations(&list, Some("pluginsList")).unwrap(),
    Vec::<String>::new()
  );
  let doctor = plugins_json(home.path(), &["doctor", "--host", "claude"]).unwrap();
  assert_eq!(
    violations(&doctor, Some("pluginsDoctor")).unwrap(),
    Vec::<String>::new(),
    "{doctor}"
  );
  assert_eq!(doctor["namespace"], "plugins");
  assert_eq!(doctor["verb"], "doctor");
  let remove = plugins_json(home.path(), &["remove", "jev", "--yes", "--host", "claude"]).unwrap();
  assert_eq!(
    violations(&remove, Some("pluginsHost")).unwrap(),
    Vec::<String>::new()
  );
  // The stub host accepts the removal.
  assert_eq!(remove["steps"][0]["removed"], true);
  assert_ne!(
    violations(&json!([{ "name": "x" }]), Some("pluginsList")).unwrap(),
    Vec::<String>::new()
  );
}
