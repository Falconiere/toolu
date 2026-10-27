use std::time::Duration;

use super::{Pin, ensure};
use crate::args::Approval;
use crate::host::adapter::Adapter;
use crate::step::Status;
use crate::support::{Sandbox, catalog, names};
use crate::verbs::{Driver, install, update};

pub(super) const TAG: &str = "v8.2.0";

/// Codex 0.164.0's refusal to add a source that differs from the registered one.
const REFUSAL: &str = "Error: marketplace 'toolu' is already added from a different source; remove it before adding this source";

/// A `codex` that keeps the `toolu` marketplace in `codex-marketplace`
/// (`none`, `git -` unpinned, `git <tag>`, or `local <dir>`) and answers its
/// marketplace commands the way Codex 0.164.0 does: a git add replaces a
/// local source and refuses a different git source. Flag files make it fail:
/// `codex-fail-pin` every pinned add, `codex-fail-pin-fresh` a pinned add with
/// no marketplace (after a remove), `codex-fail-restore` the unpinned add,
/// `codex-fail-remove` the remove, `codex-fail-list` the listing,
/// `codex-bad-list` the listing's shape, `codex-fail-plugin` a plugin add.
pub(super) fn codex(sb: &Sandbox, state: &str) {
  let root = sb.root();
  let root = root.display();
  sb.write("codex-marketplace", &format!("{state}\n"));
  let body = format!(
    r#"echo "$*" >> {root}/codex.log
state=$(cat {root}/codex-marketplace)
for word in "$@"; do last=$word; done
case "$*" in
"plugin marketplace add Falconiere/toolu --ref "*)
  if [ -e {root}/codex-fail-pin ]; then echo "Error: cannot fetch $last" >&2; exit 1; fi
  if [ "$state" = none ] && [ -e {root}/codex-fail-pin-fresh ]; then echo "Error: cannot fetch $last" >&2; exit 1; fi
  case "$state" in
  none|local*) echo "git $last" > {root}/codex-marketplace ;;
  "git $last") echo "Marketplace \`toolu\` is already added." ;;
  *) echo "{REFUSAL}" >&2; exit 1 ;;
  esac ;;
"plugin marketplace add Falconiere/toolu")
  if [ -e {root}/codex-fail-restore ]; then echo "Error: network down" >&2; exit 1; fi
  if [ "$state" = none ]; then echo "git -" > {root}/codex-marketplace; fi ;;
"plugin marketplace remove toolu")
  if [ -e {root}/codex-fail-remove ]; then echo "Error: config is read-only" >&2; exit 1; fi
  echo none > {root}/codex-marketplace ;;
"plugin marketplace list --json")
  if [ -e {root}/codex-fail-list ]; then echo "error: unrecognized subcommand 'marketplace'" >&2; exit 2; fi
  if [ -e {root}/codex-bad-list ]; then echo '[]'; exit 0; fi
  case "$state" in
  none) echo '{{"marketplaces":[]}}' ;;
  local*) echo '{{"marketplaces":[{{"name":"toolu","marketplaceSource":{{"sourceType":"local","source":"/src/toolu"}}}}]}}' ;;
  *) echo '{{"marketplaces":[{{"name":"toolu","marketplaceSource":{{"sourceType":"git","source":"https://github.com/Falconiere/toolu.git"}}}}]}}' ;;
  esac ;;
"plugin list --json") cat {root}/codex-installed.json 2>/dev/null || echo '{{"installed":[]}}' ;;
"plugin list --available --json") cat {root}/codex-available.json 2>/dev/null || echo '{{"installed":[]}}' ;;
"plugin add "*)
  if [ -e {root}/codex-fail-plugin ]; then echo "Error: plugin '$last' not found" >&2; exit 1; fi ;;
esac"#
  );
  sb.stub("codex", &body);
}

pub(super) fn driver(sb: &Sandbox) -> Driver {
  Driver {
    adapter: Adapter::Codex,
    env: sb.env(),
    timeout: Duration::from_secs(10),
    checkout: None,
    pin: Some(TAG.to_owned()),
    approval: Approval::Report,
    progress: None,
  }
}

pub(super) fn state(sb: &Sandbox) -> String {
  std::fs::read_to_string(sb.path("codex-marketplace"))
    .unwrap_or_default()
    .trim()
    .to_owned()
}

pub(super) fn marketplace_log(sb: &Sandbox) -> Vec<String> {
  sb.log("codex")
    .into_iter()
    .filter(|line| line.starts_with("plugin marketplace"))
    .collect()
}

#[test]
fn a_fresh_codex_gets_the_marketplace_pinned_to_the_tag() {
  let sb = Sandbox::new();
  codex(&sb, "none");
  assert_eq!(ensure(&driver(&sb), TAG), Ok(Pin::Pinned));
  assert_eq!(state(&sb), "git v8.2.0");
  assert_eq!(
    marketplace_log(&sb),
    [
      "plugin marketplace list --json",
      "plugin marketplace add Falconiere/toolu --ref v8.2.0"
    ]
  );
}

#[test]
fn a_marketplace_pinned_to_the_tag_is_kept_without_a_remove() {
  let sb = Sandbox::new();
  codex(&sb, "git v8.2.0");
  assert_eq!(ensure(&driver(&sb), TAG), Ok(Pin::Pinned));
  assert_eq!(
    marketplace_log(&sb),
    [
      "plugin marketplace list --json",
      "plugin marketplace add Falconiere/toolu --ref v8.2.0"
    ]
  );
}

#[test]
fn an_unpinned_toolu_marketplace_is_removed_and_added_again_pinned() {
  for previous in ["git -", "git v8.1.4"] {
    let sb = Sandbox::new();
    codex(&sb, previous);
    assert_eq!(ensure(&driver(&sb), TAG), Ok(Pin::Repinned), "{previous}");
    assert_eq!(state(&sb), "git v8.2.0");
    assert_eq!(
      marketplace_log(&sb),
      [
        "plugin marketplace list --json",
        "plugin marketplace add Falconiere/toolu --ref v8.2.0",
        "plugin marketplace remove toolu",
        "plugin marketplace add Falconiere/toolu --ref v8.2.0",
      ]
    );
  }
}

#[test]
fn a_toolu_marketplace_from_elsewhere_is_left_alone_and_named() {
  let sb = Sandbox::new();
  codex(&sb, "local /src/toolu");
  let Ok(Pin::Foreign(hint)) = ensure(&driver(&sb), TAG) else {
    panic!("a local marketplace was pinned");
  };
  assert!(hint.contains("/src/toolu (local)"), "{hint}");
  assert_eq!(state(&sb), "local /src/toolu");
  assert_eq!(marketplace_log(&sb), ["plugin marketplace list --json"]);
}

#[test]
fn a_failed_pinned_add_after_the_remove_restores_the_unpinned_marketplace() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  sb.write("codex-fail-pin-fresh", "");
  let Err(fail) = ensure(&driver(&sb), TAG) else {
    panic!("a failed pin passed");
  };
  assert_eq!(
    fail.message,
    "cannot pin the Codex marketplace toolu to v8.2.0: Error: cannot fetch v8.2.0; restored it unpinned"
  );
  assert_eq!(state(&sb), "git -");
}

#[test]
fn a_failed_restore_is_reported_with_the_command_to_run() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  sb.write("codex-fail-pin-fresh", "");
  sb.write("codex-fail-restore", "");
  let Err(fail) = ensure(&driver(&sb), TAG) else {
    panic!("a failed pin passed");
  };
  assert!(
    fail.message.ends_with(
      "restoring it unpinned failed too: Error: network down; run `codex plugin marketplace add Falconiere/toolu`"
    ),
    "{}",
    fail.message
  );
}

#[test]
fn a_repin_adds_every_installed_plugin_again_on_update_even_when_current() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  let listing = r#"{"installed":[{"name":"jev","version":"8.2.0"}]}"#;
  sb.write("codex-installed.json", listing);
  sb.write("codex-available.json", listing);
  let Ok(steps) = update::run(&driver(&sb), &catalog(), &[], false) else {
    panic!("the update failed outright");
  };
  assert_eq!(steps.len(), 1);
  assert_eq!(
    (steps[0].status, steps[0].detail.as_str()),
    (Status::Updated, "re-pinned to v8.2.0")
  );
  assert!(sb.log("codex").contains(&"plugin add jev@toolu".to_owned()));
}

#[test]
fn a_pinned_update_leaves_a_current_plugin_alone() {
  let sb = Sandbox::new();
  codex(&sb, "git v8.2.0");
  let listing = r#"{"installed":[{"name":"jev","version":"8.2.0"}]}"#;
  sb.write("codex-installed.json", listing);
  sb.write("codex-available.json", listing);
  let Ok(steps) = update::run(&driver(&sb), &catalog(), &[], false) else {
    panic!("the update failed outright");
  };
  assert_eq!(steps[0].status, Status::Current);
  assert!(!sb.log("codex").contains(&"plugin add jev@toolu".to_owned()));
}

#[test]
fn a_repin_on_install_adds_an_installed_plugin_again() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  sb.write(
    "codex-installed.json",
    r#"{"installed":[{"name":"jev","version":"8.2.0"}]}"#,
  );
  let Ok(steps) = install::run(&driver(&sb), &catalog(), &names(&["jev"]), None, false) else {
    panic!("the install failed outright");
  };
  assert_eq!(
    (steps[0].status, steps[0].detail.as_str()),
    (Status::Updated, "re-pinned to v8.2.0")
  );
}

#[test]
fn a_dry_run_runs_no_marketplace_command() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  let Ok(_steps) = update::run(&driver(&sb), &catalog(), &names(&["jev"]), true) else {
    panic!("the dry run failed");
  };
  assert!(marketplace_log(&sb).is_empty(), "{:?}", sb.log("codex"));
  assert_eq!(state(&sb), "git -");
}
