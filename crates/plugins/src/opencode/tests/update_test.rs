use crate::args::{Scope, Verb};
use crate::support::{Sandbox, TARGET, global, local, opencode, plugins_of};

#[test]
fn a_tuple_keeps_its_options_and_a_current_entry_is_left_alone() {
  let sb = Sandbox::new();
  sb.write(
    &global("opencode.jsonc"),
    "// pinned\n{ \"plugin\": [[\"@toolu/opencode@1.0.0\", { \"x\": 1 }], \"other\"] }\n",
  );
  let run = opencode(&sb, Verb::Update, &["jev"], None, false).expect("update");
  assert!(
    run
      .text
      .contains(&format!("updated @toolu/opencode@1.0.0 -> {TARGET}")),
    "{}",
    run.text
  );
  let text = sb.read(&global("opencode.jsonc"));
  assert!(text.starts_with("// pinned\n"));
  assert_eq!(
    plugins_of(&text),
    serde_json::json!([[TARGET, {"x": 1}], "other"])
  );
  let again = opencode(&sb, Verb::Update, &[], None, false).expect("again");
  assert!(
    again.text.contains(&format!("current at {TARGET}")),
    "{}",
    again.text
  );
}

#[test]
fn nothing_configured_fails_and_an_unknown_name_is_a_usage_error() {
  let sb = Sandbox::new();
  let none =
    opencode(&sb, Verb::Update, &[], None, false).map_err(|fail| (fail.exit.code(), fail.message));
  assert_eq!(
    none,
    Err((
      1,
      "@toolu/opencode is not configured in OpenCode, so there is nothing to update".to_owned()
    ))
  );
  let scoped =
    opencode(&sb, Verb::Update, &[], Some(Scope::Project), false).map_err(|fail| fail.message);
  assert!(scoped.is_err_and(|message| message.contains("(project scope)")));
  sb.write(
    &local("opencode.json"),
    "{ \"plugin\": [\"@toolu/opencode@1\"] }\n",
  );
  let unknown =
    opencode(&sb, Verb::Update, &["nope"], None, false).map_err(|fail| fail.exit.code());
  assert_eq!(unknown, Err(64));
}
