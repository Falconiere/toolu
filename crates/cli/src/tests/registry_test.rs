use std::collections::BTreeSet;

use super::{Action, BUILTIN, NAMESPACES, find, is_leaf_plugin};

#[test]
fn every_top_level_name_is_unique() {
  let names: Vec<String> = NAMESPACES
    .iter()
    .map(|namespace| (namespace.command)().get_name().to_owned())
    .collect();
  let unique: BTreeSet<&String> = names.iter().collect();
  assert_eq!(unique.len(), names.len(), "{names:?}");
}

#[test]
fn hook_and_commands_are_the_special_actions() {
  let hook = find("hook").unwrap();
  assert!(matches!(hook.action, Action::Hook));
  assert_eq!(hook.owner, "toolu");
  let commands = find("commands").unwrap();
  assert!(matches!(commands.action, Action::Commands));
  assert_eq!(commands.owner, BUILTIN);
  assert!(matches!(find("plugins").unwrap().action, Action::Verb(_)));
  assert!(find("nope").is_none());
}

#[test]
fn renamed_namespaces_keep_their_plugin_as_owner() {
  let owners: Vec<&str> = ["review", "babysit", "epic"]
    .iter()
    .map(|name| find(name).unwrap().owner)
    .collect();
  assert_eq!(owners, ["toolu-review", "pr-babysit", "epic-orchestrator"]);
}

#[test]
fn only_plugins_other_than_toolu_are_leaves() {
  assert!(is_leaf_plugin("jev"));
  assert!(is_leaf_plugin("ast-grep"));
  assert!(!is_leaf_plugin("toolu"));
  assert!(!is_leaf_plugin(BUILTIN));
}
