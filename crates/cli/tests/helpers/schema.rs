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
fn violations(document: &Value, def: Option<&str>) -> Res<Vec<String>> {
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
    (&["epic", "status", "402", "--json"], "epicStatus"),
    (&["--json", "--version"], "version"),
    (&["--json", "--help"], "help"),
    (&["--json", "epic", "--help"], "help"),
    (&["--json", "epic", "planned"], "planned"),
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
fn every_namespace_s_json_run_is_its_own_valid_document() {
  for name in namespaces().unwrap() {
    let (args, def) = match name.as_str() {
      "hook" | "commands" | "doctor" | "ledger" | "config" | "status" | "setup" | "jev"
      | "review" | "ast-grep" => continue,
      "brainstorm" | "delivery-flow" => (vec!["--json", name.as_str()], "guide"),
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
