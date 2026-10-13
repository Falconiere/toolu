use crate::args::Verb;
use crate::support::{Sandbox, TARGET, enabled_names, global, local, opencode, selection};

#[test]
fn unconfigured_lists_nothing_installed() {
  let sb = Sandbox::new();
  let report = opencode(&sb, Verb::List, &[], None, false).expect("list");
  assert!(report.text.starts_with("@toolu/opencode: not configured\n"), "{}", report.text);
  assert!(report.json.as_array().is_some_and(|rows| rows.iter().all(|row| row["installed"] == false)));
}

#[test]
fn a_project_pin_and_selection_win_and_skills_false_switches_a_plugin_off() {
  let sb = Sandbox::new();
  sb.write(&global("opencode.json"), &format!("{{ \"plugin\": [\"{TARGET}\"] }}\n"));
  assert_eq!(enabled_names(&sb).len(), crate::support::catalog().plugins.len());
  sb.write(&local("opencode.json"), "{ \"plugin\": [\"@toolu/opencode@9.9.9\"] }\n");
  sb.write(&local(".opencode/toolu/plugins.json"), &selection(&["jev", "ast-grep"]));
  sb.write(&local(".opencode/toolu.config.json"), r#"{"skills":{"ast-grep":false}}"#);
  let report = opencode(&sb, Verb::List, &[], None, false).expect("list");
  assert!(report.text.contains("@toolu/opencode: @toolu/opencode@9.9.9 in "), "{}", report.text);
  assert!(report.text.contains("selection: "), "{}", report.text);
  assert_eq!(enabled_names(&sb), ["jev"]);
  let jev = report.json.as_array().and_then(|rows| rows.iter().find(|row| row["name"] == "jev").cloned());
  assert_eq!(jev.map(|row| row["version"].clone()), Some(serde_json::json!("9.9.9")));
}
