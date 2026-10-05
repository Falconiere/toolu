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

fn edited(edit: impl FnOnce(&mut Value)) -> Vec<String> {
  let mut after = real();
  edit(&mut after);
  breaks(&real(), &after)
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
