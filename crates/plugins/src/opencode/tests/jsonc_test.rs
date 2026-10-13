use std::path::Path;

use toolu_protocol::exit::Exit;

use super::{add_plugin_key, new_config_text, parse_config, read_config_file, remove_element, remove_key, replace_spec};
use crate::support::Sandbox;

fn plugins(text: &str) -> serde_json::Value {
  match parse_config(Path::new("t"), text) {
    Ok(map) => map.get("plugin").cloned().unwrap_or_default(),
    Err(fail) => panic!("{}: {text}", fail.message),
  }
}

#[test]
fn a_tuple_spec_is_replaced_keeping_its_options_and_every_other_byte() {
  let tuple = "{ \"plugin\": [[\"@toolu/opencode@1.0.0\", { \"x\": 1 }]] }\n";
  let swapped = replace_spec(tuple, 0, true, "@toolu/opencode@2.0.0");
  assert_eq!(swapped, "{ \"plugin\": [[\"@toolu/opencode@2.0.0\", { \"x\": 1 }]] }\n");
  let plain = "{\n  // pinned\n  \"plugin\": [\"x\", \"@toolu/opencode@1\"]\n}\n";
  assert_eq!(replace_spec(plain, 1, false, "@toolu/opencode@2"), plain.replace("@toolu/opencode@1", "@toolu/opencode@2"));
  assert_eq!(replace_spec(plain, 5, false, "y"), plain);
  assert_eq!(replace_spec("{", 0, false, "y"), "{");
}

#[test]
fn the_plugin_key_is_added_and_removed_through_the_cst() {
  let added = add_plugin_key("{\n  // mine\n  \"share\": \"disabled\"\n}\n", "@toolu/opencode@1.0.0");
  assert!(added.contains("// mine"), "{added}");
  assert_eq!(plugins(&added), serde_json::json!(["@toolu/opencode@1.0.0"]));
  let removed = remove_key(&added, "plugin");
  assert!(removed.contains("// mine") && !removed.contains("plugin"), "{removed}");
  assert_eq!(remove_key("{}", "plugin"), "{}");
  assert_eq!(plugins(&remove_element("{ \"plugin\": [\"a\", \"b\"] }", "plugin", 1)), serde_json::json!(["a"]));
}

#[test]
fn a_new_config_holds_the_schema_and_the_entry() {
  assert_eq!(
    new_config_text("@toolu/opencode@7.11.0"),
    "{\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"plugin\": [\n    \"@toolu/opencode@7.11.0\"\n  ]\n}\n"
  );
}

#[test]
fn absent_malformed_and_non_object_files() {
  let sb = Sandbox::new();
  let absent = read_config_file(&sb.path("none.json"));
  assert_eq!(absent.map(|file| (file.text, file.data)), Ok((None, None)));
  let bad = sb.write("bad.jsonc", "{ \"plugin\": [ \"a\" ");
  let Err(fail) = read_config_file(&bad) else {
    panic!("malformed JSONC was read");
  };
  assert_eq!(fail.exit, Exit::Failure);
  assert!(fail.message.contains("bad.jsonc is not valid JSONC"), "{}", fail.message);
  assert!(fail.message.ends_with("; nothing was written"));
  let list = sb.write("list.json", "[1]");
  assert!(read_config_file(&list).is_err_and(|fail| fail.message.contains("must hold a JSON object")));
  let single = sb.write("single.json", "{ 'plugin': [] }");
  assert!(read_config_file(&single).is_err());
  let dir = sb.path("home");
  assert!(read_config_file(&dir).is_err_and(|fail| fail.message.starts_with("cannot read ")));
  let commented = sb.write("ok.jsonc", "// c\n{ \"plugin\": [\"a\",], }\n");
  assert!(read_config_file(&commented).is_ok_and(|file| file.data.is_some()));
}
