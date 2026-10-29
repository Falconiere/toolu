use super::super::checks::Status;
use super::super::inventory::{Inventory, Plugin};
use super::{check, check_on};
use toolu_runtime::env::Env;

fn env(path: &str) -> Env {
  Env::from_pairs([("PATH", path)])
}

fn named(names: &[&str]) -> Inventory {
  Inventory {
    plugins: names
      .iter()
      .map(|name| Plugin {
        name: (*name).to_owned(),
        version: None,
        root: None,
        muse: None,
      })
      .collect(),
    warning: None,
  }
}

#[test]
fn a_missing_required_tool_fails_with_its_hint() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("git"), "").unwrap();
  let inventory = named(&["pr-babysit"]);
  let check = check(&inventory, &env(dir.path().to_str().unwrap()));
  assert_eq!(check.status, Status::Fail);
  assert!(
    check.summary.starts_with("pr-babysit needs gh"),
    "{}",
    check.summary
  );
  assert_eq!(
    check.hint.as_deref(),
    Some("install the GitHub CLI: https://cli.github.com")
  );
  assert_eq!(check.details["missing"][0]["tool"], "gh");
  assert_eq!(check.details["missing"][0]["required"], true);
}

#[test]
fn a_missing_optional_tool_warns_when_required_tools_exist() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("git"), "").unwrap();
  std::fs::write(dir.path().join("gh"), "").unwrap();
  let inventory = named(&["pr-babysit"]);
  let check = check(&inventory, &env(dir.path().to_str().unwrap()));
  assert_eq!(check.status, Status::Warn);
  assert_eq!(check.summary, "pr-babysit needs herdr or tmux or hc");
  assert_eq!(check.hint.as_deref(), Some("install herdr, hc or tmux"));
}

#[test]
fn tmux_satisfies_the_herdr_row() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("git"), "").unwrap();
  std::fs::write(dir.path().join("gh"), "").unwrap();
  std::fs::write(dir.path().join("tmux"), "").unwrap();
  let inventory = named(&["epic-orchestrator"]);
  let check = check(&inventory, &env(dir.path().to_str().unwrap()));
  assert_eq!(check.status, Status::Ok);
}

#[test]
fn the_sandbox_wants_lima_on_macos_only() {
  let inventory = named(&["sandbox"]);
  let mac = check_on(&inventory, &env("/nonexistent"), "macos");
  assert_eq!(mac.status, Status::Warn);
  assert_eq!(mac.summary, "sandbox needs limactl");
  assert_eq!(
    mac.hint.as_deref(),
    Some("install Lima 2 for toolu sandbox: brew install lima")
  );
  let linux = check_on(&inventory, &env("/nonexistent"), "linux");
  assert_eq!(linux.status, Status::Ok);
}

#[test]
fn an_unlisted_plugin_requires_nothing() {
  let inventory = named(&["not-a-toolu-plugin"]);
  let check = check(&inventory, &env("/nonexistent"));
  assert_eq!(check.status, Status::Ok);
}
