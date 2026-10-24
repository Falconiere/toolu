//! `@toolu/plugins` is a deprecation notice. These tests run the real `node`
//! on `tools/toolu-cli/npm/cli.js`. Every invocation exits 69, names the
//! native install commands, and does not run a binary.

#[path = "helpers/npm.rs"]
mod npm;

use npm::{native_stub, repo, run, temp, wrapper};

const NOTICE: &str = "\
@toolu/plugins is deprecated. Install the native binary, then install plugins with it:
  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
  brew install falconiere/tap/toolu
  toolu plugins install
See https://github.com/Falconiere/toolu/blob/main/docs/install.md
";

#[test]
fn the_manifest_ships_the_notice_command_and_no_binaries() {
  let text = std::fs::read_to_string(repo().join("tools/toolu-cli/npm/package.json")).unwrap();
  let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
  assert_eq!(
    manifest["bin"],
    serde_json::json!({ "toolu-plugins": "cli.js" })
  );
  assert_eq!(manifest["files"], serde_json::json!(["cli.js"]));
  assert!(manifest.get("scripts").is_none(), "{manifest}");
}

#[test]
fn install_does_not_run_a_binary_named_by_toolu_bin() {
  let (_dir, root) = temp().unwrap();
  let log = root.join("stub.log");
  let stub = native_stub(&root.join("toolu"), &log, "echo should-not-run").unwrap();
  let ran = run(
    &wrapper(),
    &["install", "rust-quality", "--host", "codex"],
    &[
      ("TOOLU_BIN", stub.to_str().unwrap()),
      ("PATH", "/usr/bin:/bin"),
    ],
    &root,
  )
  .unwrap();
  assert_eq!(ran.code, Some(69), "{ran:?}");
  assert_eq!(ran.stdout, "");
  assert_eq!(ran.stderr, NOTICE);
  assert!(!log.exists(), "the stub ran: {ran:?}");
}

#[test]
fn version_and_no_arguments_print_the_notice_and_nothing_else() {
  let (_dir, root) = temp().unwrap();
  for args in [&["--version"][..], &["-v"][..], &[][..]] {
    let ran = run(&wrapper(), args, &[("PATH", "/usr/bin:/bin")], &root).unwrap();
    assert_eq!(ran.code, Some(69), "{args:?} {ran:?}");
    assert_eq!(ran.stdout, "", "{args:?}");
    assert_eq!(ran.stderr, NOTICE, "{args:?}");
  }
}
