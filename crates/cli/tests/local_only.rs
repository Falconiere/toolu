//! Removed remote-worker namespaces are rejected by the native CLI.

#[test]
fn local_only_removed_namespaces_are_usage_errors() {
  let resources = tempfile::tempdir().unwrap();
  for name in ["machines", "relay", "delegate", "peers", "herdr-app"] {
    let output = assert_cmd::Command::new(env!("CARGO_BIN_EXE_toolu"))
      .args([name, "--help"])
      .env("TOOLU_RESOURCE_HOME", resources.path())
      .output()
      .unwrap();
    assert_eq!(output.status.code(), Some(64), "{name}");
    assert_eq!(output.stdout, b"", "{name}");
    assert!(
      String::from_utf8(output.stderr)
        .unwrap()
        .contains(&format!("unrecognized subcommand '{name}'")),
      "{name}"
    );
  }
}
