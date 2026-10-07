//! `toolu ledger` black-box (#421, AC-8): each verb on a real repository, its
//! exit codes, and every `--json` document against its `commands.schema.json`
//! definition.

#[path = "helpers/ledger.rs"]
mod ledger;

use std::process::Output;

use ledger::{Project, Res};
use serde_json::{Value, json};

/// Standard output as exactly one JSON document.
fn one_document(output: &Output) -> Res<Value> {
  let mut documents = serde_json::Deserializer::from_slice(&output.stdout).into_iter::<Value>();
  let first = documents.next().ok_or("stdout holds no JSON document")??;
  if documents.next().is_some() {
    return Err("stdout holds more than one JSON document".into());
  }
  Ok(first)
}

/// `toolu <args>` in the repository.
fn toolu(project: &Project, args: &[&str]) -> Res<Output> {
  Ok(project.command(&project.root).args(args).output()?)
}

/// The errors of `document` against `$defs/<def>` of the binary's own schema.
fn violations(project: &Project, document: &Value, def: &str) -> Res<Vec<String>> {
  let schema = one_document(&toolu(project, &["commands", "--schema"])?)?;
  let wrapped = json!({
    "$schema": schema.get("$schema"),
    "$defs": schema.get("$defs"),
    "$ref": format!("#/$defs/{def}"),
  });
  let validator = jsonschema::validator_for(&wrapped).map_err(|err| err.to_string())?;
  Ok(
    validator
      .iter_errors(document)
      .map(|err| err.to_string())
      .collect(),
  )
}

const PLAN: &str = "# P\n\n**Status:** Approved   **Spec:** spec.md\n\n## Steps (machine-readable)\n\n```json\n[{\"id\":\"s1\",\"title\":\"one\",\"check\":\"true\",\"ac_refs\":[\"AC-1\"]},{\"id\":\"s2\",\"title\":\"two\",\"check\":\"false\"}]\n```\n";
const SPEC: &str = "# S\n\n**Status:** Approved\n\n## Acceptance criteria\n- **AC-1:** a\n";

#[test]
fn every_json_document_validates_against_its_definition() {
  let project = Project::new().unwrap();
  project.write("plan.md", PLAN).unwrap();
  project.write("spec.md", SPEC).unwrap();
  for (args, def, code) in [
    (&["--json", "ledger", "run", "plan.md"][..], "ledgerRun", 1),
    (&["--json", "ledger", "status"], "ledgerStatus", 1),
    (&["--json", "ledger", "path"], "ledgerPath", 0),
    (&["--json", "ledger", "root"], "ledgerRoot", 0),
    (&["--json", "ledger", "self-test"], "ledgerSelfTest", 0),
    (&["--json", "ledger", "preflight", "plan.md"], "empty", 0),
    (&["--json", "ledger", "verdict", "status"], "verdict", 1),
    (&["--json", "ledger", "run", "absent.md"], "error", 2),
    (&["--json", "ledger", "frobnicate"], "error", 64),
  ] {
    let output = toolu(&project, args).unwrap();
    assert_eq!(
      output.status.code(),
      Some(code),
      "{args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    let document = one_document(&output).unwrap();
    assert_eq!(
      violations(&project, &document, def).unwrap(),
      Vec::<String>::new(),
      "{args:?}: {document}"
    );
  }
}

#[test]
fn text_output_and_exits_follow_the_typescript_cli() {
  let project = Project::new().unwrap();
  project.write("plan.md", PLAN).unwrap();
  project.write("spec.md", SPEC).unwrap();
  let run = toolu(&project, &["ledger", "run", "plan.md"]).unwrap();
  assert_eq!(run.status.code(), Some(1));
  assert_eq!(
    String::from_utf8_lossy(&run.stdout),
    "plan-ledger feat_x: 1/2 fresh-green, next=s2\n"
  );
  let path = toolu(&project, &["ledger", "path"]).unwrap();
  assert_eq!(
    String::from_utf8_lossy(&path.stdout),
    format!("{}\n", project.ledger().display())
  );
  let flag = toolu(&project, &["ledger", "--self-test"]).unwrap();
  assert_eq!(
    String::from_utf8_lossy(&flag.stdout),
    "plan-ledger --self-test: ok\n"
  );
  let verdict = toolu(&project, &["ledger", "verdict", "json"]).unwrap();
  let report: Value = serde_json::from_slice(&verdict.stdout).unwrap();
  assert_eq!(report["overall"], "blocked");
  assert!(String::from_utf8_lossy(&verdict.stdout).starts_with("{\n  \"version\": 1,\n"));
  let usage = toolu(&project, &["ledger", "verdict", "table"]).unwrap();
  assert_eq!(usage.status.code(), Some(64));
  let quiet = toolu(&project, &["--quiet", "ledger", "self-test"]).unwrap();
  assert_eq!(quiet.stderr, b"");
  project.sh("git checkout -q --orphan unborn").unwrap();
  let unborn = toolu(&project, &["ledger", "path"]).unwrap();
  assert_eq!(unborn.status.code(), Some(2));
  let outside = project
    .command(&project.home)
    .args(["ledger", "root"])
    .output()
    .unwrap();
  assert_eq!(
    String::from_utf8_lossy(&outside.stderr),
    "plan-ledger: not in a git repo\n"
  );
}
