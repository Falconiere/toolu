use super::command;

fn names(command: &clap::Command) -> Vec<&str> {
  command
    .get_subcommands()
    .map(clap::Command::get_name)
    .collect()
}

#[test]
fn the_tree_is_a_valid_clap_definition() {
  command().debug_assert();
}

#[test]
fn every_namespace_is_a_top_level_command() {
  assert_eq!(
    names(&command()),
    [
      "hook",
      "ledger",
      "debug",
      "setup",
      "doctor",
      "config",
      "status",
      "serve",
      "ts-quality",
      "python-quality",
      "rust-quality",
      "ast-grep",
      "brainstorm",
      "delivery-flow",
      "review",
      "jev",
      "statusline",
      "babysit",
      "epic",
      "commands",
      "plugins",
    ]
  );
}

#[test]
fn leaf_plugins_get_a_hidden_hook_verb_and_their_plugin_alias() {
  let root = command();
  let review = root.find_subcommand("review").unwrap();
  let hook = review.find_subcommand("hook").unwrap();
  assert!(hook.is_hide_set());
  assert_eq!(
    review.get_visible_aliases().collect::<Vec<_>>(),
    ["toolu-review"]
  );
  let jev = root.find_subcommand("jev").unwrap();
  assert_eq!(jev.get_visible_aliases().count(), 0);
  assert_eq!(names(jev), ["planned", "hook"]);
  let ledger = root.find_subcommand("ledger").unwrap();
  assert_eq!(
    names(ledger),
    [
      "run",
      "status",
      "preflight",
      "path",
      "root",
      "self-test",
      "verdict"
    ]
  );
  assert_eq!(names(root.find_subcommand("commands").unwrap()).len(), 0);
}

#[test]
fn the_long_help_documents_streams_and_every_exit_code() {
  let help = command().render_long_help().to_string();
  assert!(help.contains("With --json, stdout is exactly one JSON document"));
  for line in [
    "0   success",
    "1   failure",
    "2   blocked",
    "64  usage",
    "69  unavailable",
    "75  tempfail",
  ] {
    assert!(help.contains(line), "{line}\n{help}");
  }
}
