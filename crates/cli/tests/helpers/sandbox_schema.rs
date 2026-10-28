//! `toolu sandbox` documents against `$defs/sandboxStatus`, `sandboxSetup` and
//! `sandboxVm`: the binary's own output on this machine's default backend and on
//! the host backend, and a document the schema must refuse.

use std::process::Output;

use serde_json::{Value, json};

use crate::cli::{Res, one_document};
use crate::schema::violations;

/// `toolu --json sandbox <args>` with a private home whose config holds `config`.
fn sandbox(config: Option<&str>, args: &[&str]) -> Res<(Output, Value)> {
  let home = tempfile::tempdir()?;
  let user = home.path().join("user");
  std::fs::create_dir_all(&user)?;
  if let Some(text) = config {
    std::fs::write(user.join("toolu.config.json"), text)?;
  }
  let mut command = assert_cmd::Command::new(crate::cli::TOOLU);
  command
    .args(["--json", "sandbox"])
    .args(args)
    .current_dir(home.path())
    .env_clear()
    .env("HOME", home.path())
    .env("PATH", std::env::var("PATH")?)
    .env("TOOLU_USER_CONFIG_DIR", &user);
  if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
    command.env("LLVM_PROFILE_FILE", profile);
  }
  let output = command.output()?;
  let document = one_document(&output)?;
  Ok((output, document))
}

const HOST: &str = r#"{"version":1,"sandbox":{"backend":"host"}}"#;

#[test]
fn every_sandbox_document_validates() {
  let cases: [(Option<&str>, &[&str], &str); 6] = [
    (None, &["status"], "sandboxStatus"),
    (Some(HOST), &["status"], "sandboxStatus"),
    (Some(HOST), &["setup"], "sandboxSetup"),
    (Some(HOST), &["start", "--restart"], "sandboxVm"),
    (Some(HOST), &["stop"], "sandboxVm"),
    (Some(HOST), &["delete", "--yes"], "sandboxVm"),
  ];
  for (config, args, def) in cases {
    let (_output, document) = sandbox(config, args).unwrap();
    assert_eq!(
      violations(&document, Some(def)).unwrap(),
      Vec::<String>::new(),
      "{args:?}: {document}"
    );
    assert_eq!(document["namespace"], "sandbox");
  }
}

#[test]
fn a_state_the_sandbox_never_reports_is_refused() {
  let (_output, mut status) = sandbox(Some(HOST), &["status"]).unwrap();
  status["state"] = json!("paused");
  assert_ne!(
    violations(&status, Some("sandboxStatus")).unwrap(),
    Vec::<String>::new()
  );
  let (_output, mut vm) = sandbox(Some(HOST), &["stop"]).unwrap();
  vm["verb"] = json!("status");
  assert_ne!(
    violations(&vm, Some("sandboxVm")).unwrap(),
    Vec::<String>::new()
  );
}
