use super::super::checks::Status;
use super::super::inventory::Inventory;
use super::check;
use toolu_runtime::env::Env;

fn env(path: &str) -> Env {
  Env::from_pairs([("PATH", path)])
}

#[test]
fn a_missing_required_tool_fails_with_its_hint() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("git"), "").unwrap();
  let inventory = Inventory::named(&["pr-babysit"]);
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
  let inventory = Inventory::named(&["pr-babysit"]);
  let check = check(&inventory, &env(dir.path().to_str().unwrap()));
  assert_eq!(check.status, Status::Warn);
  assert_eq!(check.summary, "pr-babysit needs herdr");
  assert_eq!(check.hint.as_deref(), Some("install herdr"));
}

#[test]
fn an_unlisted_plugin_requires_nothing() {
  let inventory = Inventory::named(&["not-a-toolu-plugin"]);
  let check = check(&inventory, &env("/nonexistent"));
  assert_eq!(check.status, Status::Ok);
}
