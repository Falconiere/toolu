use serde_json::json;

use super::{Row, host_json, install, install_json, list, planned, remove, rows_json, update};
use crate::host::Target;
use crate::step::{Status, Step};
use crate::support::names;

fn step(name: &str, status: Status, detail: &str, argv: &[&str]) -> Step {
  Step::new(name, status, detail, names(argv))
}

#[test]
fn an_install_names_every_plugin_with_its_mark_or_lists_the_planned_commands() {
  let steps = vec![
    step("toolu", Status::Installed, "installed", &["claude", "x"]),
    step("ast-grep", Status::Skew, "installed at 6.4.0, marketplace offers 6.5.0", &["claude", "y"]),
    step("jev", Status::Failed, "boom", &["claude", "z"]),
  ];
  let text = install(&[(Target::Claude, steps.clone())], false);
  assert_eq!(
    text,
    "  + toolu            installed\n  ! ast-grep         installed at 6.4.0, marketplace offers 6.5.0\n  x jev              boom\n"
  );
  let dry = install(&[(Target::Claude, steps)], true);
  assert_eq!(dry, "Planned commands (nothing was run):\n  claude x\n  claude y\n  claude z\n");
  assert_eq!(install(&[(Target::Claude, Vec::new())], false), "");
}

#[test]
fn several_hosts_get_a_labelled_section_each() {
  let one = |name: &str| vec![step(name, Status::Skipped, "dry run", &["claude", "plugin", "install", name])];
  let single = install(&[(Target::Claude, one("toolu"))], true);
  assert!(!single.contains("claude:"));
  let both = install(&[(Target::Claude, one("toolu")), (Target::Codex, one("ast-grep"))], true);
  assert!(both.starts_with("claude:\nPlanned commands"), "{both}");
  assert!(both.contains("\n\ncodex:\nPlanned commands"), "{both}");
}

#[test]
fn opencode_dry_runs_print_planned_changes_and_refusals() {
  let steps = vec![
    step("@toolu/opencode", Status::Installed, "added", &[]).planned("add @toolu/opencode@1 to /g/opencode.json"),
    step("jev", Status::Failed, "required by pr-babysit; remove them too", &[]),
  ];
  let text = install(&[(Target::Opencode, steps.clone())], true);
  assert_eq!(
    text,
    "Planned changes (nothing was written):\n  add @toolu/opencode@1 to /g/opencode.json\n  x jev              required by pr-babysit; remove them too\n"
  );
  assert_eq!(planned(&[]), "Planned changes (nothing was written):\n  (no changes)\n");
  assert_eq!(planned(&steps), text);
}

#[test]
fn list_remove_and_update_reports() {
  let rows = vec![
    Row { name: "toolu".to_owned(), installed: true, version: Some("6.5.0".to_owned()), enabled: true },
    Row { name: "jev".to_owned(), installed: true, version: None, enabled: false },
    Row { name: "ast-grep".to_owned(), installed: false, version: None, enabled: false },
  ];
  assert_eq!(
    list(&rows),
    "  = toolu            6.5.0\n  = jev              ? (disabled)\n  - ast-grep         not installed\n"
  );
  let removed = vec![step("ast-grep", Status::Removed, "removed", &[]), step("jev", Status::Failed, "nope", &[])];
  assert_eq!(remove(&removed), "  - ast-grep         removed\n  x jev              nope\n");
  let updated = vec![step("ast-grep", Status::Current, "current at 6.5.0", &[])];
  assert_eq!(update(&updated), "  = ast-grep         current at 6.5.0\n");
}

#[test]
fn json_documents_keep_the_typescript_shapes() {
  let rows = vec![
    Row { name: "toolu".to_owned(), installed: true, version: Some("7.11.0".to_owned()), enabled: true },
    Row { name: "jev".to_owned(), installed: false, version: None, enabled: false },
  ];
  assert_eq!(
    rows_json(&rows),
    json!([
      {"name": "toolu", "installed": true, "version": "7.11.0", "enabled": true},
      {"name": "jev", "installed": false, "enabled": false}
    ])
  );
  let removed = vec![step("jev", Status::Removed, "disabled", &[]).planned("disable jev in /s")];
  assert_eq!(
    host_json(Target::Opencode, &removed, true),
    json!({"host": "opencode", "steps": [{"name": "jev", "removed": true, "detail": "disabled", "argv": [], "plan": "disable jev in /s"}]})
  );
  let installed = vec![step("toolu", Status::Failed, "boom", &["claude", "plugin"])];
  assert_eq!(
    install_json(&[(Target::Claude, installed)], false),
    json!({"dryRun": false, "hosts": [{"host": "claude", "steps": [{"name": "toolu", "outcome": "failed", "detail": "boom", "argv": ["claude", "plugin"]}]}]})
  );
}
