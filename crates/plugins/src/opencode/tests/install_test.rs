use crate::args::{Scope, Verb};
use crate::opencode::jsonc::new_config_text;
use crate::support::{Sandbox, TARGET, enabled_names, enabled_of, global, local, opencode, plugins_of, selection};

#[test]
fn a_bare_install_adds_one_entry_and_no_selection_and_a_rerun_changes_nothing() {
  let sb = Sandbox::new();
  let first = opencode(&sb, Verb::Install, &[], None, false).expect("install");
  assert!(!first.failed);
  assert_eq!(sb.read(&global("opencode.json")), new_config_text(TARGET));
  assert!(!sb.path(&global("toolu/plugins.json")).exists());
  let again = opencode(&sb, Verb::Install, &[], None, false).expect("again");
  assert!(again.text.contains("already configured"), "{}", again.text);
  assert_eq!(sb.read(&global("opencode.json")), new_config_text(TARGET));
}

#[test]
fn a_named_install_on_a_clean_profile_enables_exactly_its_closure() {
  let sb = Sandbox::new();
  opencode(&sb, Verb::Install, &["jev"], None, false).expect("jev");
  assert_eq!(enabled_of(&sb.read(&global("toolu/plugins.json"))), ["jev"]);
  opencode(&sb, Verb::Install, &["pr-babysit"], None, false).expect("pr-babysit");
  assert_eq!(enabled_names(&sb), ["jev", "pr-babysit", "toolu"]);
}

#[test]
fn an_existing_pin_is_kept_and_reported_as_skew() {
  let sb = Sandbox::new();
  let pinned = "{ \"plugin\": [\"@toolu/opencode@1.0.0\"] }\n";
  sb.write(&global("opencode.json"), pinned);
  let run = opencode(&sb, Verb::Install, &[], None, false).expect("install");
  assert!(!run.failed);
  assert!(run.text.contains(&format!("this CLI installs {TARGET}")), "{}", run.text);
  assert!(run.text.contains("toolu plugins update --host opencode"), "{}", run.text);
  assert_eq!(sb.read(&global("opencode.json")), pinned);
}

#[test]
fn without_scope_install_enables_in_the_governing_project_selection() {
  let sb = Sandbox::new();
  opencode(&sb, Verb::Install, &["ast-grep"], Some(Scope::Project), false).expect("project");
  let run = opencode(&sb, Verb::Install, &["jev"], None, false).expect("jev");
  let project = sb.path(&local(".opencode/toolu/plugins.json"));
  assert!(run.text.contains(&format!("enabled in {}", project.display())), "{}", run.text);
  assert_eq!(enabled_of(&sb.read(&local(".opencode/toolu/plugins.json"))), ["ast-grep", "jev"]);
  assert!(!sb.path(&global("toolu/plugins.json")).exists());
}

#[test]
fn layering_notes_name_the_shadowing_and_lifted_files() {
  let sb = Sandbox::new();
  let shadowed = format!("{{ \"plugin\": [\"{TARGET}\"] }}\n");
  sb.write(&global("config.json"), &shadowed);
  sb.write(&global("opencode.json"), "{ \"plugin\": [\"other-plugin\"] }\n");
  let run = opencode(&sb, Verb::Install, &[], None, false).expect("install");
  assert!(run.text.contains(&format!("added {TARGET} to {}", sb.path(&global("opencode.json")).display())));
  assert_eq!(plugins_of(&sb.read(&global("opencode.json"))), serde_json::json!(["other-plugin", TARGET]));
  assert_eq!(sb.read(&global("config.json")), shadowed);

  let reset = Sandbox::new();
  reset.write(&local("opencode.json"), "{ \"plugin\": [] }\n");
  let run = opencode(&reset, Verb::Install, &[], None, false).expect("install");
  let note = format!("no effect here: {} sets an empty plugin list", reset.path(&local("opencode.json")).display());
  assert!(run.text.contains(&note) && !run.failed, "{}", run.text);

  let lift = Sandbox::new();
  lift.write(&global("opencode.json"), "{ \"plugin\": [\"other-global-plugin\"] }\n");
  lift.write(&local("opencode.json"), "{ \"plugin\": [] }\n");
  let run = opencode(&lift, Verb::Install, &[], Some(Scope::Project), false).expect("install");
  assert!(run.text.contains("no longer empties the plugin list, so plugins from earlier config files load here again"));
  assert_eq!(plugins_of(&lift.read(&local("opencode.json"))), serde_json::json!([TARGET]));
}

#[test]
fn a_global_edit_a_project_selection_overrides_says_so_and_skills_off_stays_off() {
  let sb = Sandbox::new();
  sb.write(&local(".opencode/toolu/plugins.json"), &selection(&["jev"]));
  sb.write(&global("opencode.json"), &format!("{{ \"plugin\": [\"{TARGET}\"] }}\n"));
  sb.write(&global("toolu/plugins.json"), &selection(&["jev"]));
  sb.write(&local(".opencode/toolu.config.json"), r#"{"skills":{"ast-grep":false}}"#);
  let run = opencode(&sb, Verb::Install, &["ast-grep"], Some(Scope::User), false).expect("install");
  let governs = sb.path(&local(".opencode/toolu/plugins.json"));
  assert!(run.text.contains(&format!("no effect in this project: {} governs it", governs.display())), "{}", run.text);
  assert!(run.text.contains("(skills.ast-grep is false in"), "{}", run.text);
  assert_eq!(enabled_of(&sb.read(&global("toolu/plugins.json"))), ["jev", "ast-grep"]);
}

#[test]
fn a_new_project_entry_goes_to_the_root_opencode_json_or_an_existing_jsonc() {
  let sb = Sandbox::new();
  opencode(&sb, Verb::Install, &[], Some(Scope::Project), false).expect("install");
  assert_eq!(sb.read(&local("opencode.json")), new_config_text(TARGET));
  let jsonc = Sandbox::new();
  jsonc.write(&local("opencode.jsonc"), "// mine\n{ \"share\": \"disabled\" }\n");
  opencode(&jsonc, Verb::Install, &[], Some(Scope::Project), false).expect("install");
  let text = jsonc.read(&local("opencode.jsonc"));
  assert!(text.starts_with("// mine\n") && text.contains(TARGET), "{text}");
}
