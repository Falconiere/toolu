//! The pin's failure paths: what is left alone, what fails the verb, and the
//! hint a failed plugin install carries.

use super::tests::{TAG, driver, marketplace_log, state};
use super::{Pin, ensure};
use crate::codex_stub::codex;
use crate::step::Status;
use crate::support::{Sandbox, catalog, names};
use crate::verbs::{install, update};

#[test]
fn an_unreadable_marketplace_list_fails_and_adds_nothing() {
  for flag in ["codex-fail-list", "codex-bad-list"] {
    let sb = Sandbox::new();
    codex(&sb, "local /src/toolu");
    sb.write(flag, "");
    let Err(fail) = ensure(&driver(&sb), TAG) else {
      panic!("{flag}: an unreadable listing passed");
    };
    assert!(
      fail.message.starts_with(
        "cannot pin the Codex marketplace toolu to v8.2.0: codex plugin marketplace list --json: "
      ),
      "{flag}: {}",
      fail.message
    );
    assert!(
      flag == "codex-bad-list" || fail.message.ends_with("(0.164.0 or later); update Codex"),
      "{flag}: {}",
      fail.message
    );
    assert_eq!(
      marketplace_log(&sb),
      ["plugin marketplace list --json"],
      "{flag}"
    );
    assert_eq!(state(&sb), "local /src/toolu", "{flag}");
  }
}

#[test]
fn a_failed_pinned_add_of_toolus_marketplace_that_is_no_refusal_removes_nothing() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  sb.write("codex-fail-pin", "");
  let Err(fail) = ensure(&driver(&sb), TAG) else {
    panic!("a failed pinned add passed");
  };
  assert_eq!(
    fail.message,
    "cannot pin the Codex marketplace toolu to v8.2.0: Error: cannot fetch v8.2.0"
  );
  assert_eq!(
    marketplace_log(&sb),
    [
      "plugin marketplace list --json",
      "plugin marketplace add Falconiere/toolu --ref v8.2.0"
    ]
  );
  assert_eq!(state(&sb), "git -");
}

#[test]
fn a_failed_remove_fails_the_pin_and_keeps_the_marketplace() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  sb.write("codex-fail-remove", "");
  let Err(fail) = ensure(&driver(&sb), TAG) else {
    panic!("a failed remove passed");
  };
  assert_eq!(
    fail.message,
    "cannot pin the Codex marketplace toolu to v8.2.0: Error: config is read-only"
  );
  assert_eq!(state(&sb), "git -");
}

#[test]
fn no_marketplace_and_a_failed_add_hint_on_a_failed_install_and_fail_an_update() {
  let sb = Sandbox::new();
  codex(&sb, "none");
  sb.write("codex-fail-pin", "");
  assert_eq!(
    ensure(&driver(&sb), TAG),
    Ok(Pin::Absent("Error: cannot fetch v8.2.0".to_owned()))
  );
  sb.write("codex-fail-plugin", "");
  let Ok(steps) = install::run(&driver(&sb), &catalog(), &names(&["jev"]), None, false) else {
    panic!("the install failed outright");
  };
  assert_eq!(
    (steps[0].status, steps[0].detail.as_str()),
    (
      Status::Failed,
      "Error: plugin 'jev@toolu' not found; the marketplace add also failed: Error: cannot fetch v8.2.0"
    )
  );
  sb.write(
    "codex-installed.json",
    r#"{"installed":[{"name":"jev","version":"8.2.0"}]}"#,
  );
  let Err(fail) = update::run(&driver(&sb), &catalog(), &[], false, None) else {
    panic!("an update with no marketplace to pin passed");
  };
  assert_eq!(
    fail.message,
    "cannot pin the Codex marketplace toolu to v8.2.0: Error: cannot fetch v8.2.0"
  );
}

#[test]
fn a_spelling_of_toolus_origin_without_dot_git_is_still_toolus() {
  let sb = Sandbox::new();
  codex(&sb, "git -");
  let listing = r#"{"marketplaces":[{"name":"toolu","marketplaceSource":{"sourceType":"git","source":"https://github.com/Falconiere/toolu/"}}]}"#;
  sb.write("listing.json", listing);
  let root = sb.root();
  let body = format!(
    "case \"$*\" in \"plugin marketplace list --json\") cat {}/listing.json ;; *) exec {}/bin/codex-real \"$@\" ;; esac",
    root.display(),
    root.display()
  );
  std::fs::rename(sb.path("bin/codex"), sb.path("bin/codex-real")).expect("move");
  sb.stub("codex", &body);
  assert_eq!(ensure(&driver(&sb), TAG), Ok(Pin::Repinned));
  assert_eq!(state(&sb), "git v8.2.0");
}
