use std::path::PathBuf;

use super::{entry_json, run};
use crate::Verdict;
use crate::options::Options;
use toolu_protocol::launcher::{Target, hook};

const GOLDEN: &str =
  include_str!("../../../core/protocol/src/tests/fixtures/launcher-pre-tool-use.txt");

fn options(args: &[&str], timeout: Option<&str>) -> Options {
  Options {
    root: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")),
    files: args.iter().map(PathBuf::from).collect(),
    timeout: timeout.map(str::to_owned),
    ..Options::default()
  }
}

#[test]
fn a_known_plugin_entry_prints() {
  let verdict = run(&options(&["toolu", "PreToolUse", "pre-tools"], Some("600"))).unwrap();
  assert_eq!(verdict, Verdict::Clean);
}

#[test]
fn the_entry_carries_the_golden_command_and_the_default_timeout() {
  let target = Target {
    plugin: "toolu",
    event: "PreToolUse",
    name: "pre-tools",
  };
  let entry: serde_json::Value =
    serde_json::from_str(&entry_json(&hook(&target, 60).unwrap())).unwrap();
  assert_eq!(entry["type"], "command");
  assert_eq!(entry["command"], GOLDEN);
  assert_eq!(entry["timeout"], 60);
  assert_eq!(
    entry["commandWindows"],
    hook(&target, 60).unwrap().command_windows
  );
}

#[test]
fn bad_timeouts_names_plugins_and_arity_are_errors() {
  let pre = ["toolu", "PreToolUse", "pre-tools"];
  for timeout in ["0", "601", "abc", "-1"] {
    assert!(run(&options(&pre, Some(timeout))).is_err(), "{timeout}");
  }
  assert!(run(&options(&["toolu", "PreToolUse", "Bad_Name"], None)).is_err());
  let unknown = run(&options(&["no-such-plugin", "PreToolUse", "x"], None)).unwrap_err();
  assert_eq!(unknown, "no plugins/no-such-plugin directory");
  assert!(run(&options(&["toolu", "PreToolUse"], None)).is_err());
}
