use crate::args::{Scope, Verb};
use crate::support::{Sandbox, TARGET, global, opencode, opencode_with, request};

#[test]
fn scope_local_and_a_bad_package_override_are_usage_errors() {
  let sb = Sandbox::new();
  let local =
    opencode(&sb, Verb::Install, &[], Some(Scope::Local), false).map_err(|fail| fail.exit.code());
  assert_eq!(local.map(|_| ()), Err(64));
  let evil = sb.env().with("TOOLU_OPENCODE_PACKAGE", "evil@1.0.0");
  let run = opencode_with(&sb, &evil, &request(Verb::Install, &[], None, false))
    .map_err(|fail| fail.exit.code());
  assert_eq!(run.map(|_| ()), Err(64));
  assert!(!sb.path(&global("opencode.json")).exists());
}

#[test]
fn dry_runs_print_planned_changes_and_write_nothing() {
  let sb = Sandbox::new();
  let install = opencode(&sb, Verb::Install, &["jev"], None, true).expect("install");
  assert!(
    install
      .text
      .starts_with("Planned changes (nothing was written):\n  add "),
    "{}",
    install.text
  );
  assert!(
    install.text.contains("  enable jev in "),
    "{}",
    install.text
  );
  assert!(!sb.path(&global("opencode.json")).exists());
  sb.write(
    &global("opencode.json"),
    "{ \"plugin\": [\"@toolu/opencode@1\"] }\n",
  );
  let update = opencode(&sb, Verb::Update, &[], None, true).expect("update");
  assert!(
    update
      .text
      .contains(&format!("update @toolu/opencode@1 -> {TARGET}")),
    "{}",
    update.text
  );
  let remove = opencode(&sb, Verb::Remove, &["toolu"], None, true).expect("remove");
  assert!(
    remove.text.contains("remove @toolu/opencode from"),
    "{}",
    remove.text
  );
  assert_eq!(
    sb.read(&global("opencode.json")),
    "{ \"plugin\": [\"@toolu/opencode@1\"] }\n"
  );
}
