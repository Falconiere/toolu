use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::time::Duration;

use serde_json::json;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::super::checks::Status;
use super::super::inventory::plugins_check;
use super::{collect, collect_with_budget, parse_inspect, parse_listing};

fn toolu_root() -> String {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins/toolu")
    .display()
    .to_string()
}

#[test]
fn list_keeps_only_packages_from_the_toolu_repository() {
  let root = toolu_root();
  let body = json!({"plugins":[
    {"active":true,"plugin":{"id":"toolu"},
      "record":{"cache_path":root,"enabled":true,"trust":"user-local"}},
    {"active":true,"plugin":{"id":"other"},
      "record":{"cache_path":"/missing/other","enabled":true,"trust":"user-local"}}
  ]});
  let selected = parse_listing(&body.to_string()).unwrap();
  assert_eq!(selected.len(), 1);
  assert_eq!(selected[0].id, "toolu");
  assert!(parse_listing("{}").is_err());
  assert!(parse_listing("not json").is_err());
}

#[test]
fn inspect_distinguishes_unapproved_approved_and_unknown_hooks() {
  let unapproved = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"review_needed"}]});
  let pending = parse_inspect(&unapproved.to_string()).unwrap();
  assert_eq!(pending.pending_hooks, vec!["probe"]);
  assert_eq!(pending.approval, "needs-approval");
  let rejected = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"trusted_disabled"}]});
  let disabled = parse_inspect(&rejected.to_string()).unwrap();
  assert_eq!(disabled.pending_hooks, vec!["probe"]);
  assert_eq!(disabled.approval, "needs-approval");
  let approved = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"trusted_enabled"}]});
  assert_eq!(
    parse_inspect(&approved.to_string()).unwrap().approval,
    "approved"
  );
  let agent = json!({"runtime_capabilities":[{"candidate":{"scoped_definition_id":"toolu/implementer"},"status":"review_needed"}]});
  let pending_agent = parse_inspect(&agent.to_string()).unwrap();
  assert_eq!(pending_agent.approval, "needs-approval");
  assert_eq!(pending_agent.pending_hooks, Vec::<String>::new());
  let unknown = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"future_status"}]});
  assert!(parse_inspect(&unknown.to_string()).is_err());
  assert!(parse_inspect("{}").is_err());
}

#[test]
fn sandboxed_trust_is_unknown_not_inert() {
  let (_temp, roots, listing, inspection) = command_fixture();
  let muse_dir = roots.env().home().join(".config").join("muse");
  std::fs::create_dir_all(&muse_dir).unwrap();
  std::fs::set_permissions(&muse_dir, std::fs::Permissions::from_mode(0o000)).unwrap();
  write_list(&listing, true, true);
  write_inspect(&inspection, "review_needed");
  let check = plugins_check(&collect(&roots));
  std::fs::set_permissions(&muse_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
  assert_eq!(check.status, Status::Warn);
  assert!(!check.summary.contains("inert"), "{}", check.summary);
  assert!(
    check.summary.contains("unknown inside the sandbox"),
    "{}",
    check.summary
  );
  assert!(
    check.summary.contains("muse plugins inspect"),
    "{}",
    check.summary
  );
  assert_eq!(check.details["plugins"][0]["approval"], "sandboxed");
}

#[test]
fn collect_reads_installed_capabilities_through_the_muse_command() {
  let (_temp, roots, listing, inspection) = command_fixture();
  write_list(&listing, true, true);
  write_inspect(&inspection, "review_needed");
  let incomplete = plugins_check(&collect_with_budget(&roots, Duration::ZERO));
  assert_eq!(incomplete.status, Status::Warn);
  assert!(incomplete.summary.contains("inventory is incomplete"));
  let pending = plugins_check(&collect(&roots));
  assert_eq!(pending.status, Status::Warn);
  assert!(pending.summary.contains("muse plugins approve toolu"));
  assert_eq!(
    pending.details["plugins"][0]["pendingHooks"][0],
    "pre-tools"
  );

  write_inspect(&inspection, "trusted_enabled");
  let approved = plugins_check(&collect(&roots));
  assert_eq!(approved.status, Status::Ok);
  assert_eq!(approved.details["plugins"][0]["approval"], "approved");

  write_list(&listing, false, false);
  let disabled = plugins_check(&collect(&roots));
  assert_eq!(disabled.status, Status::Warn);
  assert!(disabled.summary.contains("muse plugins enable toolu"));

  std::fs::write(&inspection, "not JSON").unwrap();
  let malformed = plugins_check(&collect(&roots));
  assert_eq!(malformed.status, Status::Warn);
  assert!(
    malformed
      .summary
      .contains("cannot inspect Muse plugin toolu")
  );
}

fn command_fixture() -> (
  tempfile::TempDir,
  Roots,
  std::path::PathBuf,
  std::path::PathBuf,
) {
  let temp = tempfile::tempdir().unwrap();
  let bin = temp.path().join("bin");
  std::fs::create_dir_all(&bin).unwrap();
  let command = bin.join("muse");
  std::fs::write(
    &command,
    "#!/bin/sh\ncase \"$*\" in\n  'plugins list --json') cat \"$MUSE_LIST\";;\n  'plugins inspect toolu --json') cat \"$MUSE_INSPECT\";;\n  *) exit 1;;\nesac\n",
  )
  .unwrap();
  std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755)).unwrap();
  let listing = temp.path().join("list.json");
  let inspection = temp.path().join("inspect.json");
  let env = Env::from_pairs([
    ("HOME", temp.path().to_str().unwrap()),
    ("PATH", format!("{}:/usr/bin:/bin", bin.display()).as_str()),
    ("MUSE_LIST", listing.to_str().unwrap()),
    ("MUSE_INSPECT", inspection.to_str().unwrap()),
  ]);
  let roots = Roots::new(env, Some(Host::Muse));
  (temp, roots, listing, inspection)
}

fn write_list(path: &Path, enabled: bool, active: bool) {
  let data = json!({"plugins":[{"plugin":{"id":"toolu","version":"8.0.4"},
    "record":{"cache_path":toolu_root(),"enabled":enabled},"active":active}]});
  std::fs::write(path, data.to_string()).unwrap();
}

fn write_inspect(path: &Path, status: &str) {
  let data = json!({"runtime_capabilities":[{"candidate":
    {"kind":"hook","capability_id":"pre-tools"},"status":status}]});
  std::fs::write(path, data.to_string()).unwrap();
}
