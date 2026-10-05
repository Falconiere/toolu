use serde_json::{Value, json};

use super::{breaks, verdict};

/// The repository's command tree, as `cargo xtask docs-cli` wrote it from the binary.
fn real() -> Value {
  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/cli/commands.json");
  serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The index of the top-level command `name`.
fn at(tree: &Value, name: &str) -> usize {
  tree["commands"]
    .as_array()
    .unwrap()
    .iter()
    .position(|command| command["name"] == name)
    .unwrap()
}

/// What the real tree breaks once `edit` has changed it.
fn edited(edit: impl FnOnce(&mut Value)) -> Vec<String> {
  let mut after = real();
  edit(&mut after);
  breaks(&real(), &after)
}

/// What the real tree breaks of a past tree `edit` has changed.
fn since(edit: impl FnOnce(&mut Value)) -> Vec<String> {
  let mut before = real();
  edit(&mut before);
  breaks(&before, &real())
}

/// The flag `long` of the root, or of the top-level command `command`.
fn flag_of<'a>(tree: &'a mut Value, command: Option<&str>, long: &str) -> &'a mut Value {
  let node = match command {
    Some(name) => {
      let index = at(tree, name);
      &mut tree["commands"][index]
    }
    None => tree,
  };
  node["flags"]
    .as_array_mut()
    .unwrap()
    .iter_mut()
    .find(|flag| flag["long"] == long)
    .unwrap()
}

#[test]
fn an_unchanged_tree_breaks_nothing() {
  assert_eq!(breaks(&real(), &real()), Vec::<String>::new());
}

#[test]
fn removing_a_command_alias_or_flag_breaks_callers() {
  assert_eq!(
    edited(|tree| {
      let hook = at(tree, "hook");
      tree["commands"].as_array_mut().unwrap().remove(hook);
    }),
    ["command `toolu hook` removed"]
  );
  assert_eq!(
    edited(|tree| {
      let index = at(tree, "epic");
      tree["commands"][index]["aliases"] = json!([]);
    }),
    ["alias `epic-orchestrator` of `toolu epic` removed"]
  );
  assert_eq!(
    edited(|tree| {
      let hook = at(tree, "hook");
      tree["commands"][hook]["flags"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    }),
    ["flag `--event` of `toolu hook` removed"]
  );
}

#[test]
fn shrinking_a_flag_breaks_callers() {
  assert_eq!(
    edited(|tree| tree["flags"][1]["short"] = Value::Null),
    ["short flag of `--quiet` on `toolu` removed or changed"]
  );
  assert_eq!(
    edited(|tree| {
      tree["flags"][2]["possibleValues"]
        .as_array_mut()
        .unwrap()
        .pop();
    }),
    ["value \"hermes\" of `--host` on `toolu` removed"]
  );
  assert_eq!(
    edited(|tree| {
      let index = at(tree, "hook");
      tree["commands"][index]["flags"][0]["required"] = json!(true);
    }),
    ["flag `--event` on `toolu hook` became required"]
  );
  assert_eq!(
    edited(|tree| tree["flags"][0]["takesValue"] = json!(true)),
    ["`--json` on `toolu` changed whether it takes a value"]
  );
}

#[test]
fn an_alias_that_moved_to_another_command_is_removed_from_this_one() {
  assert_eq!(
    edited(|tree| {
      let review = at(tree, "review");
      tree["commands"][review]["aliases"] = json!([]);
      let jev = at(tree, "jev");
      tree["commands"][jev]["aliases"] = json!(["toolu-review"]);
    }),
    ["alias `toolu-review` of `toolu review` removed"]
  );
}

#[test]
fn a_global_flag_that_stops_being_global_breaks_callers() {
  assert_eq!(
    edited(|tree| flag_of(tree, None, "json")["global"] = json!(false)),
    ["flag `--json` on `toolu` is no longer global"]
  );
  // A flag that was never global may stay local, and a local one may become global.
  assert_eq!(
    since(|tree| flag_of(tree, None, "json")["global"] = json!(false)),
    Vec::<String>::new()
  );
}

#[test]
fn a_free_form_value_that_gains_a_list_narrows_the_input() {
  assert_eq!(
    edited(|tree| flag_of(tree, None, "config-dir")["possibleValues"] = json!(["a", "b"])),
    ["`--config-dir` on `toolu` now accepts only listed values"]
  );
  // A flag that takes no value has nothing to narrow.
  assert_eq!(
    edited(|tree| flag_of(tree, None, "json")["possibleValues"] = json!(["x"])),
    Vec::<String>::new()
  );
}

#[test]
fn positionals_cannot_disappear_or_become_required() {
  assert_eq!(
    since(|tree| {
      let hook = at(tree, "hook");
      tree["commands"][hook]["args"][0]["required"] = json!(false);
    }),
    ["argument NAME of `toolu hook` became required"]
  );
  assert_eq!(
    edited(|tree| {
      let hook = at(tree, "hook");
      tree["commands"][hook]["args"] = json!([]);
    }),
    ["argument NAME of `toolu hook` removed"]
  );
  let optional = json!({ "name": "MORE", "required": false, "multiple": false, "help": "" });
  assert_eq!(
    edited(|tree| {
      let hook = at(tree, "hook");
      tree["commands"][hook]["args"]
        .as_array_mut()
        .unwrap()
        .push(optional);
    }),
    Vec::<String>::new()
  );
}

#[test]
fn a_change_to_a_nested_verb_is_found_beneath_its_namespace() {
  assert_eq!(
    edited(|tree| {
      let quality = at(tree, "ts-quality");
      let hook = tree["commands"][quality]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .position(|verb| verb["name"] == "hook")
        .unwrap();
      tree["commands"][quality]["commands"][hook]["flags"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    }),
    ["flag `--event` of `toolu ts-quality hook` removed"]
  );
  assert_eq!(
    since(|tree| {
      let quality = at(tree, "ts-quality");
      tree["commands"][quality]["commands"]
        .as_array_mut()
        .unwrap()
        .retain(|verb| verb["name"] != "hook");
    }),
    Vec::<String>::new()
  );
}

#[test]
fn exit_codes_and_required_arguments_are_part_of_the_contract() {
  assert_eq!(
    edited(|tree| {
      tree["exitCodes"].as_array_mut().unwrap().pop();
    }),
    ["exit code 75 (tempfail) removed or renamed"]
  );
  let required = json!({ "name": "TARGET", "required": true, "multiple": false, "help": "" });
  assert_eq!(
    edited(|tree| {
      let index = at(tree, "commands");
      tree["commands"][index]["args"] = json!([required]);
    }),
    ["new required argument TARGET on `toolu commands`"]
  );
  let flag = json!({ "long": "must", "short": null, "valueName": "X", "takesValue": true,
    "required": true, "global": false, "hidden": false, "possibleValues": [], "help": "" });
  assert_eq!(
    edited(|tree| {
      let index = at(tree, "commands");
      tree["commands"][index]["flags"] = json!([flag]);
    }),
    [
      "flag `--schema` of `toolu commands` removed",
      "new required flag `--must` on `toolu commands`"
    ]
  );
}

#[test]
fn placeholders_and_renames_that_keep_an_alias_break_nothing() {
  assert_eq!(
    edited(|tree| {
      let epic = at(tree, "epic");
      tree["commands"][epic]["commands"]
        .as_array_mut()
        .unwrap()
        .retain(|verb| verb["name"] != "planned");
    }),
    Vec::<String>::new()
  );
  assert_eq!(
    edited(|tree| {
      let babysit = at(tree, "babysit");
      tree["commands"][babysit]["name"] = json!("pr");
      tree["commands"][babysit]["aliases"] = json!(["babysit", "pr-babysit"]);
    }),
    Vec::<String>::new()
  );
}

#[test]
fn a_break_needs_a_hook_protocol_bump_and_a_breaking_title() {
  let found = vec!["command `toolu hook` removed".to_owned()];
  let same = verdict(&found, 1, 1, Some("feat(cli): x"));
  assert_eq!(same.len(), 1);
  assert!(
    same[0].starts_with("command `toolu hook` removed — "),
    "{same:?}"
  );
  assert_eq!(
    verdict(&found, 1, 2, Some("feat(cli)!: x")),
    Vec::<String>::new()
  );
  assert_eq!(
    verdict(&found, 1, 2, Some("refactor!: x")),
    Vec::<String>::new()
  );
  assert_eq!(verdict(&found, 1, 2, None), Vec::<String>::new());
  let titled = verdict(&found, 1, 2, Some("feat(cli): x"));
  assert!(
    titled[0].contains("lacks the conventional `!`"),
    "{titled:?}"
  );
  assert_eq!(
    verdict(&[], 2, 1, None),
    ["hookProtocol decreased from 2 to 1"]
  );
  assert_eq!(verdict(&[], 1, 1, Some("feat: x")), Vec::<String>::new());
}

#[test]
fn only_a_bang_before_the_colon_marks_a_title_breaking() {
  let found = vec!["command `toolu hook` removed".to_owned()];
  for title in [
    "feat: drop foo!",
    "fix: a!: b",
    "feat(a!): x",
    "feat !: x",
    "!",
  ] {
    let titled = verdict(&found, 1, 2, Some(title));
    assert_eq!(titled.len(), 1, "{title}: {titled:?}");
    assert!(
      titled[0].contains("lacks the conventional `!`"),
      "{title}: {titled:?}"
    );
  }
  for title in ["feat!: x", "fix(a-b)!: x"] {
    assert_eq!(
      verdict(&found, 1, 2, Some(title)),
      Vec::<String>::new(),
      "{title}"
    );
  }
}
