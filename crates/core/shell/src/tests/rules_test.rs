//! `shell-rules.test.ts`: a rule names a command and the tokens it must carry;
//! it matches any simple command in the line, through wrappers, prefixes and
//! subshells, but never across two commands.

use super::matches_rule;
use crate::analyze;

fn hits(source: &str, rule: &str) -> bool {
  analyze(source)
    .commands
    .iter()
    .any(|command| matches_rule(command, rule))
}

#[test]
fn a_multi_word_rule_matches_the_command_wherever_it_sits() {
  for source in [
    "node -e 1",
    "cd /tmp && node -e 1",
    "FOO=1 node -e 1",
    "(node -e 1)",
    "sudo node -e 1",
    "/usr/local/bin/node -e 1",
    "bash -lc 'node -e 1'",
    "eval \"node -e 1\"",
  ] {
    assert!(hits(source, "node -e"), "{source}");
  }
}

#[test]
fn a_rule_does_not_match_prose_another_command_or_two_commands() {
  for source in [
    "git commit -m \"fix node -e failure\"",
    "node script.js",
    "mynode -e 1",
    "node script.js && echo -e x",
    "echo node -e",
  ] {
    assert!(!hits(source, "node -e"), "{source}");
  }
  assert!(!hits("bun install && node -e \"1\"", "bun -e"));
  assert!(hits("cd crate && cargo test", "cargo test"));
  assert!(hits("cargo --verbose test", "cargo test"));
}

#[test]
fn a_single_token_rule_names_the_command_or_a_wrapper() {
  assert!(hits("biome check .", "biome"));
  assert!(hits("ls -la", "ls"));
  assert!(!hits("false; echo also", "ls"));
  assert!(hits("sudo rm -rf x", "sudo"));
  assert!(hits("sudo rm -rf x", "rm -rf"));
  assert!(!hits("./tools/x/check.sh", "tools/x/check.sh"));
  assert!(hits("tools/x/check.sh --fix", "tools/x/check.sh"));
}

#[test]
fn an_empty_rule_matches_nothing() {
  assert!(!hits("ls", ""));
  assert!(!hits("ls", "   "));
  assert!(!hits("$x -e", "node -e"));
}

#[test]
fn escaped_separators_and_brace_names_cannot_hide_a_rule() {
  assert!(hits("node -\\\ne x", "node -e"));
  assert!(hits("bash -c 'node'\\ '-e x'", "node -e"));
  assert!(!hits("{node,} -e x", "node -e"));
  assert_eq!(analyze("{node,} -e x").commands[0].argv[0], None);
}
