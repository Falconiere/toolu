use serde_json::Value;

use super::{Origin, judge};

/// The repository's real command tree.
fn tree() -> Value {
  let text = std::fs::read_to_string(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/cli/commands.json"
  ))
  .unwrap();
  serde_json::from_str(&text).unwrap()
}

fn check(command: &str, origin: Origin) -> Option<String> {
  let words: Vec<String> = command.split(' ').map(str::to_owned).collect();
  judge(&tree(), &words, origin)
}

fn passes(command: &str, origin: Origin) {
  assert_eq!(check(command, origin), None, "{command}");
}

fn fails(command: &str, origin: Origin, wanted: &[&str]) {
  let problem = check(command, origin).unwrap_or_else(|| panic!("{command} passed"));
  for part in wanted {
    assert!(problem.contains(part), "{command}: {problem} lacks {part}");
  }
}

#[test]
fn an_unknown_verb_names_the_closest_and_the_valid_ones() {
  for origin in [Origin::Fenced, Origin::Inline] {
    fails(
      "toolu epic strat",
      origin,
      &[
        "unknown command `strat` under `toolu epic`",
        "closest: `planned`",
        "valid: planned",
      ],
    );
  }
  fails("toolu epik start", Origin::Inline, &["closest: `epic`"]);
}

#[test]
fn an_unknown_flag_lists_the_valid_flags() {
  fails(
    "toolu --jsn epic planned",
    Origin::Fenced,
    &[
      "unknown flag `--jsn` on `toolu`",
      "--json",
      "--quiet/-q",
      "--version/-V",
      "--help/-h",
    ],
  );
  fails(
    "toolu --json=x commands",
    Origin::Fenced,
    &["`--json` on `toolu` takes no value"],
  );
  fails(
    "toolu --help=x",
    Origin::Inline,
    &["`--help` on `toolu` takes no value"],
  );
  fails("toolu --h", Origin::Inline, &["unknown flag `--h`"]);
  passes("toolu -h", Origin::Fenced);
  fails(
    "toolu epic planned -x",
    Origin::Fenced,
    &["unknown flag `-x` on `toolu epic planned`", "--json"],
  );
}

#[test]
fn global_flags_reach_every_command_and_values_are_checked() {
  passes("toolu epic planned --json -q", Origin::Fenced);
  passes("toolu --json jev planned", Origin::Fenced);
  passes("toolu --host <h> commands", Origin::Fenced);
  passes("toolu --host=$HOST commands", Origin::Fenced);
  passes("toolu commands --host codex --schema", Origin::Fenced);
  fails(
    "toolu --host bogus commands",
    Origin::Fenced,
    &["does not accept `bogus`", "claude"],
  );
  fails(
    "toolu commands --host",
    Origin::Fenced,
    &["`--host` on `toolu commands` needs a value"],
  );
  passes("toolu commands --host", Origin::Inline);
}

#[test]
fn prose_mentions_pass_inline_and_fail_fenced() {
  passes("toolu hook", Origin::Inline);
  fails(
    "toolu hook",
    Origin::Fenced,
    &["`toolu hook` needs its NAME argument"],
  );
  passes("toolu doctor", Origin::Inline);
  passes("toolu doctor", Origin::Fenced);
  passes("toolu", Origin::Inline);
}

#[test]
fn todays_documented_hook_forms_pass() {
  for command in [
    "toolu hook pre-tools",
    "toolu hook pre-tools --event PreToolUse",
    "toolu hook <name>",
    "toolu <plugin> hook <name>",
    "toolu <plugin> hook <entry>",
    "toolu [<plugin>] hook <name> --event <Event> --plugin-root <root>",
    "toolu jev hook session-start --event SessionStart --plugin-root <dir>",
    "toolu pr-babysit hook --help",
    "toolu --hook-protocol",
    "toolu --version",
    "toolu -V",
    "toolu commands --json",
    "toolu epic planned",
    "toolu hook …",
  ] {
    passes(command, Origin::Fenced);
  }
}

#[test]
fn unported_namespaces_have_only_their_placeholder_verb() {
  fails(
    "toolu ledger run",
    Origin::Fenced,
    &["unknown command `run` under `toolu ledger`"],
  );
  fails(
    "toolu ast-grep run",
    Origin::Inline,
    &["under `toolu ast-grep`"],
  );
}

#[test]
fn positional_counts_and_placeholders_in_command_position() {
  fails(
    "toolu hook a b",
    Origin::Inline,
    &["too many arguments for `toolu hook` (takes 1)"],
  );
  fails(
    "toolu review hook a b",
    Origin::Inline,
    &["too many arguments"],
  );
  fails(
    "toolu <plugin> hook --bogus",
    Origin::Fenced,
    &["no command under `toolu` fits `<plugin> hook --bogus`"],
  );
  fails(
    "toolu commands -- x",
    Origin::Fenced,
    &["too many arguments"],
  );
  passes("toolu hook -- x", Origin::Fenced);
  passes("toolu [<plugin>] hook <name>", Origin::Fenced);
}
