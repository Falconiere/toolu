use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;

use super::{command, run};
use crate::tree;

fn script(shell: &str) -> String {
  let matches = command()
    .try_get_matches_from(["completions", shell])
    .unwrap();
  run(&matches, &Ctx::default(), &tree::command)
    .stdout
    .unwrap()
}

/// Whether the fish script offers `name` as a subcommand of `parent`.
fn fish_offers(script: &str, parent: &str, name: &str) -> bool {
  let seen = format!("__fish_toolu_using_subcommand {parent}");
  let choice = format!("-a \"{name}\"");
  script
    .lines()
    .any(|line| line.contains(&seen) && line.contains(&choice))
}

#[test]
fn bash_lists_a_visible_command_and_a_host_value() {
  let script = script("bash");
  assert!(script.contains("plugins"), "{script}");
  assert!(script.contains("claude"), "{script}");
}

#[test]
fn zsh_defines_the_toolu_command() {
  let script = script("zsh");
  assert!(
    script.contains("#compdef toolu") || script.contains("compdef _toolu toolu"),
    "{script}"
  );
}

#[test]
fn fish_completes_toolu_and_skips_a_hidden_hook_verb() {
  let script = script("fish");
  assert!(script.contains("complete -c toolu"), "{script}");
  assert!(fish_offers(&script, "jev", "noul"), "{script}");
  assert!(!fish_offers(&script, "jev", "hook"), "{script}");
}

#[test]
fn json_is_a_usage_error_with_no_script() {
  let matches = command()
    .try_get_matches_from(["completions", "bash"])
    .unwrap();
  let ctx = Ctx {
    json: true,
    ..Ctx::default()
  };
  let outcome = run(&matches, &ctx, &tree::command);
  assert_eq!(outcome.exit, Exit::Usage);
  assert_eq!(outcome.stdout, None);
  assert!(
    outcome
      .stderr
      .unwrap()
      .contains("prints a shell script, not a JSON document")
  );
}
