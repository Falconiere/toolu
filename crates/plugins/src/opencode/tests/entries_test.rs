use std::path::PathBuf;

use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{effective_entry, governing_global, package_name, plugin_array, target_spec, toolu_entries, version_of};
use crate::opencode::jsonc::{ConfigFile, parse_config};

fn file(path: &str, plugins: Option<serde_json::Value>) -> ConfigFile {
  let text = plugins.map(|plugins| serde_json::json!({ "plugin": plugins }).to_string());
  let data = text.as_deref().map(|text| parse_config(&PathBuf::from(path), text).unwrap_or_default());
  ConfigFile { path: PathBuf::from(path), text, data }
}

#[test]
fn package_names_of_scoped_unscoped_pinned_file_and_url_specs() {
  assert_eq!(package_name("@toolu/opencode@7.8.0"), "@toolu/opencode");
  assert_eq!(package_name("@toolu/opencode"), "@toolu/opencode");
  assert_eq!(package_name("@toolu/opencode@file:/tmp/a.tgz"), "@toolu/opencode");
  assert_eq!(package_name("other@1.0.0"), "other");
  assert_ne!(package_name("file:///x/toolu.ts"), "@toolu/opencode");
  assert_eq!(package_name("@scope"), "@scope");
  assert_eq!(version_of("@toolu/opencode"), "latest");
  assert_eq!(version_of("@toolu/opencode@^7.0.0"), "^7.0.0");
}

#[test]
fn the_target_spec_defaults_to_the_version_and_an_override_must_name_the_package() {
  assert_eq!(target_spec(&Env::default(), "7.8.0"), Ok("@toolu/opencode@7.8.0".to_owned()));
  let local = Env::from_pairs([("TOOLU_OPENCODE_PACKAGE", "@toolu/opencode@file:/t.tgz")]);
  assert_eq!(target_spec(&local, "7.8.0"), Ok("@toolu/opencode@file:/t.tgz".to_owned()));
  let evil = Env::from_pairs([("TOOLU_OPENCODE_PACKAGE", "evil@1.0.0")]);
  let failed = target_spec(&evil, "7.8.0").map_err(|fail| (fail.exit, fail.message));
  assert_eq!(failed, Err((Exit::Usage, "TOOLU_OPENCODE_PACKAGE must name @toolu/opencode, got: evil@1.0.0".to_owned())));
}

#[test]
fn entries_are_specs_or_tuples_of_the_package_and_a_non_array_fails() {
  let mixed = file("/g/opencode.json", Some(serde_json::json!(["x", ["@toolu/opencode@1", {"a": 1}], 3, [4], "@toolu/opencode@2"])));
  let entries = toolu_entries(&mixed).unwrap_or_default();
  let seen: Vec<(usize, &str, bool)> = entries.iter().map(|e| (e.index, e.spec.as_str(), e.tuple)).collect();
  assert_eq!(seen, [(1, "@toolu/opencode@1", true), (4, "@toolu/opencode@2", false)]);
  let text = file("/g/str.json", None);
  let text = ConfigFile { data: parse_config(&text.path, "{ \"plugin\": \"a\" }").ok(), ..text };
  let failed = plugin_array(&text).map_err(|fail| fail.message);
  assert_eq!(failed, Err("/g/str.json: \"plugin\" must be an array; nothing was written".to_owned()));
}

#[test]
fn r1_the_highest_priority_global_file_defining_plugin_replaces_the_others() {
  let global = [
    file("/g/config.json", Some(serde_json::json!(["@toolu/opencode@1.0.0"]))),
    file("/g/opencode.json", Some(serde_json::json!(["x"]))),
    file("/g/opencode.jsonc", None),
  ];
  assert_eq!(governing_global(&global).ok().flatten().map(|f| f.path.clone()), Some(PathBuf::from("/g/opencode.json")));
  assert_eq!(effective_entry(&global, &[]), Ok(None));
}

#[test]
fn r2_project_entries_concatenate_and_the_last_for_the_package_wins() {
  let global = [file("/g/config.json", None), file("/g/opencode.json", Some(serde_json::json!(["@toolu/opencode@1.0.0", "x"])))];
  let project = [file("/p/opencode.json", Some(serde_json::json!([["@toolu/opencode@2.0.0", {"a": 1}]])))];
  let entry = effective_entry(&global, &project).ok().flatten();
  assert_eq!(entry.map(|e| (e.spec, e.tuple)), Some(("@toolu/opencode@2.0.0".to_owned(), true)));
}

#[test]
fn r3_an_empty_project_array_drops_the_global_entry() {
  let global = [file("/g/opencode.json", Some(serde_json::json!(["@toolu/opencode@1.0.0"])))];
  assert_eq!(effective_entry(&global, &[file("/p/opencode.json", Some(serde_json::json!([])))]), Ok(None));
}
