use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{command, needs_stdin, run};

fn repo_plugin(name: &str) -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

fn matches(words: &[&str]) -> clap::ArgMatches {
  command()
    .try_get_matches_from(std::iter::once("opencode").chain(words.iter().copied()))
    .unwrap()
}

#[test]
fn bridge_reads_stdin_and_catalog_does_not() {
  let root = repo_plugin("toolu").display().to_string();
  let bridge = matches(&[
    "bridge",
    "--event",
    "tool.execute.before",
    "--plugin-root",
    &root,
  ]);
  assert!(needs_stdin(&bridge));
  assert!(!needs_stdin(&matches(&["catalog", "--plugin-root", &root])));
}

#[test]
fn bridge_takes_only_opencode_hook_names() {
  let err = command()
    .try_get_matches_from([
      "opencode",
      "bridge",
      "--event",
      "PreToolUse",
      "--plugin-root",
      ".",
    ])
    .unwrap_err();
  assert!(err.to_string().contains("tool.execute.before"), "{err}");
}

#[test]
fn an_unreadable_call_is_answered_on_stdout_and_fails_closed_before_a_tool() {
  let root = repo_plugin("toolu").display().to_string();
  let sub = matches(&[
    "bridge",
    "--event",
    "tool.execute.before",
    "--plugin-root",
    &root,
  ]);
  let out = run(&sub, &Ctx::default());
  assert_eq!(out.exit, Exit::Success);
  let reply: serde_json::Value = serde_json::from_str(&out.stdout.unwrap()).unwrap();
  assert_eq!(reply["action"], "deny");
  assert!(
    reply["reason"]
      .as_str()
      .unwrap()
      .contains("could not be read")
  );
}

#[test]
fn catalog_prints_the_plugin_document_or_fails() {
  let root = repo_plugin("toolu").display().to_string();
  let out = run(
    &matches(&["catalog", "--plugin-root", &root]),
    &Ctx::default(),
  );
  assert_eq!(out.exit, Exit::Success);
  let doc: serde_json::Value = serde_json::from_str(&out.stdout.unwrap()).unwrap();
  assert_eq!(doc["plugin"], "toolu");
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("skills"), "not a directory").unwrap();
  let broken = dir.path().display().to_string();
  let failed = run(
    &matches(&["catalog", "--plugin-root", &broken]),
    &Ctx::default(),
  );
  assert_eq!(failed.exit, Exit::Failure);
  assert!(failed.stderr.unwrap().contains("toolu opencode catalog"));
}
