use toolu_runtime::env::Env;

use super::{command, run};
use crate::verbs::command as epic_command;

#[test]
fn the_hosts_tree_takes_a_positional_host() {
  for line in [
    vec!["epic", "hosts", "pause", "codex"],
    vec!["epic", "hosts", "resume", "claude"],
  ] {
    assert!(
      epic_command().try_get_matches_from(&line).is_ok(),
      "{line:?}"
    );
  }
  // `--host` is a global flag elsewhere; it is not the hosts verb's argument.
  assert!(
    epic_command()
      .try_get_matches_from(["epic", "hosts", "pause", "--host", "codex"])
      .is_err()
  );
  assert!(command().try_get_matches_from(["hosts", "pause"]).is_err());
}

#[test]
fn an_unknown_host_fails_listing_the_valid_hosts() {
  let matches = command()
    .try_get_matches_from(["hosts", "pause", "nope"])
    .expect("matches");
  let outcome = run(&matches, &Env::from_pairs([("HOME", "/nonexistent")]));
  let text = format!("{outcome:?}");
  assert!(text.contains("unknown host nope"), "{text}");
  assert!(
    text.contains("use one of claude, codex, cursor, muse"),
    "{text}"
  );
}
