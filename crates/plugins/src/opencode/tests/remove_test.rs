use crate::args::{Scope, Verb};
use crate::support::{Sandbox, TARGET, enabled_of, global, local, opencode, selection};

#[test]
fn a_needed_dependency_is_refused_and_absent_names_are_no_ops() {
  let sb = Sandbox::new();
  opencode(&sb, Verb::Install, &["pr-babysit"], None, false).expect("install");
  let refused = opencode(
    &sb,
    Verb::Remove,
    &["toolu-review", "ast-grep"],
    None,
    false,
  )
  .expect("remove");
  assert!(!refused.failed, "{}", refused.text);
  assert!(
    refused.text.contains("not enabled; nothing to change"),
    "{}",
    refused.text
  );
  let needed = opencode(&sb, Verb::Remove, &["jev"], None, false);
  assert!(needed.is_ok());
  let blocked = Sandbox::new();
  opencode(&blocked, Verb::Install, &["pr-babysit"], None, false).expect("install");
  let run = opencode(&blocked, Verb::Remove, &["jev"], None, false).expect("remove");
  assert!(!run.failed);
  let core = Sandbox::new();
  opencode(&core, Verb::Install, &["delivery-flow"], None, false).expect("install");
  let refused = opencode(&core, Verb::Remove, &["pr-babysit"], None, false).expect("remove");
  assert!(refused.failed, "{}", refused.text);
  assert!(
    refused
      .text
      .contains("required by delivery-flow; remove them too"),
    "{}",
    refused.text
  );
  let both = opencode(
    &core,
    Verb::Remove,
    &["pr-babysit", "delivery-flow"],
    None,
    false,
  )
  .expect("remove");
  assert!(!both.failed, "{}", both.text);
  assert_eq!(
    enabled_of(&core.read(&global("toolu/plugins.json"))),
    ["toolu", "toolu-review", "brainstorm"]
  );
}

#[test]
fn removing_the_last_entry_deletes_the_key_and_names_the_file_that_applies_again() {
  let sb = Sandbox::new();
  sb.write(
    &global("config.json"),
    "{ \"plugin\": [\"older-plugin\"] }\n",
  );
  sb.write(
    &global("opencode.json"),
    &format!("{{ \"plugin\": [\"{TARGET}\"] }}\n"),
  );
  sb.write(&global("toolu/plugins.json"), &selection(&["jev"]));
  let run = opencode(&sb, Verb::Remove, &["toolu", "jev"], None, false).expect("remove");
  let lower = sb.path(&global("config.json"));
  assert!(
    run
      .text
      .contains(&format!("{}'s plugin list applies again", lower.display())),
    "{}",
    run.text
  );
  assert!(run.text.contains("selection kept at"), "{}", run.text);
  assert!(
    run.text.contains("removed with @toolu/opencode"),
    "{}",
    run.text
  );
  let left: serde_json::Value =
    serde_json::from_str(&sb.read(&global("opencode.json"))).expect("json");
  assert_eq!(left, serde_json::json!({}));
  let again = opencode(&sb, Verb::Remove, &["toolu"], None, false).expect("again");
  assert!(
    again.text.contains("is not configured; nothing to remove"),
    "{}",
    again.text
  );
}

#[test]
fn other_entries_stay_and_a_leaf_remove_lands_in_the_project_selection() {
  let sb = Sandbox::new();
  sb.write(
    &global("opencode.json"),
    &format!("{{\n  \"plugin\": [\n    \"keep\",\n    \"{TARGET}\"\n  ]\n}}\n"),
  );
  sb.write(
    &local(".opencode/toolu/plugins.json"),
    &selection(&["ast-grep", "jev"]),
  );
  let run = opencode(&sb, Verb::Remove, &["jev"], None, false).expect("leaf");
  assert!(run.text.contains("disabled in"), "{}", run.text);
  assert_eq!(
    enabled_of(&sb.read(&local(".opencode/toolu/plugins.json"))),
    ["ast-grep"]
  );
  opencode(&sb, Verb::Remove, &["toolu"], None, false).expect("package");
  assert_eq!(
    sb.read(&global("opencode.json")),
    "{\n  \"plugin\": [\n    \"keep\"\n  ]\n}\n"
  );
}

#[test]
fn both_scopes_need_a_scope_and_nothing_configured_removes_nothing() {
  let sb = Sandbox::new();
  let nothing = opencode(&sb, Verb::Remove, &["jev"], None, false).expect("remove");
  assert!(
    nothing
      .text
      .contains("is not configured; nothing to remove")
  );
  sb.write(
    &global("opencode.json"),
    &format!("{{ \"plugin\": [\"{TARGET}\"] }}\n"),
  );
  sb.write(
    &local("opencode.json"),
    &format!("{{ \"plugin\": [\"{TARGET}\"] }}\n"),
  );
  let ambiguous =
    opencode(&sb, Verb::Remove, &["toolu"], None, false).map_err(|fail| fail.exit.code());
  assert_eq!(ambiguous, Err(64));
  opencode(&sb, Verb::Remove, &["toolu"], Some(Scope::Project), false).expect("project");
  assert!(!sb.read(&local("opencode.json")).contains("plugin"));
}

#[test]
fn a_name_with_nothing_to_remove_is_not_reported_as_removed() {
  let sb = Sandbox::new();
  let nothing = opencode(&sb, Verb::Remove, &["toolu", "jev"], None, false).expect("remove");
  assert!(
    nothing.json["steps"]
      .as_array()
      .is_some_and(|steps| steps.len() == 2 && steps.iter().all(|step| step["removed"] == false)),
    "{}",
    nothing.json
  );
}
