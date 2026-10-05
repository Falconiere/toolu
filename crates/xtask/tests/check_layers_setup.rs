//! `cargo xtask check-layers` against real temp workspaces: the real xtask
//! binary, real `cargo metadata`: membership, the layer table and setup errors.

#[path = "helpers/layered.rs"]
mod layered;

use std::fs;

use layered::{Crate, check, krate, stderr, stdout, workspace, xtask};

fn protocol() -> Crate {
  krate("crates/core/protocol", "toolu-protocol")
}

#[test]
fn a_crate_directory_missing_from_members_fails() {
  let stray = krate("crates/stray", "toolu-stray");
  let output = check(&workspace(&[protocol()], &[stray]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/stray has a Cargo.toml but is not a workspace member\n"
  );
}

#[test]
fn an_unlistable_crate_directory_is_reported() {
  let fixture = workspace(&[krate("crates/statusline", "toolu-statusline")], &[]).unwrap();
  fs::write(fixture.root.join("crates/core"), "not a directory\n").unwrap();
  let output = check(&fixture).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(
    stderr(&output).starts_with("check-layers: cannot list crates/core: "),
    "{}",
    stderr(&output)
  );
}

#[test]
fn a_core_crate_missing_from_the_layer_table_fails() {
  let extra = krate("crates/core/extra", "toolu-extra");
  let output = check(&workspace(&[extra], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/core/extra (toolu-extra): crates/core/extra is a core crate with no \
     layer in layers.json\n"
  );
}

#[test]
fn a_member_outside_crates_fails() {
  let tool = krate("tools/helper", "helper");
  let output = check(&workspace(&[tool], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).contains("is not a crates/<name> or crates/core/<name> directory"));
}

#[test]
fn the_real_workspace_passes() {
  let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
  let output = xtask(&["check-layers", "--root", root]).unwrap();
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert!(
    stdout(&output).starts_with("check-layers: ") && stdout(&output).ends_with(" edges, ok\n"),
    "{}",
    stdout(&output)
  );
}

#[test]
fn a_capability_crate_outside_its_owner_fails() {
  let ureq = krate("crates/ureq", "ureq");
  let statusline = krate("crates/statusline", "toolu-statusline").dep("ureq", "crates/ureq");
  let output = check(&workspace(&[statusline, ureq], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(
    stderr(&output).contains("crates/statusline (toolu-statusline) depends on ureq: only toolu-http may link a `http` crate"),
    "{}",
    stderr(&output)
  );
}

#[test]
fn an_invalid_layer_table_exits_2() {
  let layered = workspace(&[protocol()], &[]).unwrap();
  let layers = layered
    .root
    .join("tooling/conventions/guardrails/rust/layers.json");
  fs::write(
    &layers,
    r#"{"core": [], "rules": [], "hub": "toolu", "binary": "cli", "tooling": "xtask", "extra": 1}"#,
  )
  .unwrap();
  let output = check(&layered).unwrap();
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).contains("invalid layer table: unknown field `extra`"),
    "{}",
    stderr(&output)
  );
  fs::remove_file(&layers).unwrap();
  assert!(
    stderr(&check(&layered).unwrap())
      .contains("cannot read tooling/conventions/guardrails/rust/layers.json")
  );
}

#[test]
fn a_failing_cargo_metadata_exits_2() {
  let layered = workspace(&[protocol()], &[]).unwrap();
  fs::write(
    layered.root.join("Cargo.toml"),
    "[workspace
",
  )
  .unwrap();
  let output = check(&layered).unwrap();
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).starts_with("xtask: cargo metadata failed"),
    "{}",
    stderr(&output)
  );
}

#[test]
fn usage_errors_exit_2() {
  for args in [
    &[][..],
    &["nope"][..],
    &["check-layers", "--root"][..],
    &["check-layers", "--x", "y"][..],
  ] {
    let output = xtask(args).unwrap();
    assert_eq!(output.status.code(), Some(2), "{args:?}");
    let text = stderr(&output);
    assert!(text.starts_with("xtask: "), "{args:?}: {text}");
    assert!(
      text.contains("usage: cargo xtask <task>"),
      "{args:?}: {text}"
    );
  }
}
