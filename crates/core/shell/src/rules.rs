//! Argv rules per simple command (`shell-rules.ts`): a rule's first token must
//! name the command and every later token must appear later in its argv. It is
//! tried on the words as written and on the unwrapped command, so `sudo node -e`
//! matches both `sudo` and `node -e`.

use crate::analysis::{ShellCommand, Word};
use crate::argv::basename;

fn argv_matches(argv: &[Word], head: &str, tail: &[&str]) -> bool {
  let Some((Some(name), rest)) = argv.split_first() else {
    return false;
  };
  let named = if head.contains('/') {
    name == head
  } else {
    basename(name) == head
  };
  named
    && tail
      .iter()
      .all(|token| rest.iter().any(|word| word.as_deref() == Some(*token)))
}

/// Whether `command` matches `rule` (`node -e`, `cargo test`, `biome`). An empty rule matches nothing.
pub fn matches_rule(command: &ShellCommand, rule: &str) -> bool {
  let mut tokens = rule.split_whitespace();
  let Some(head) = tokens.next() else {
    return false;
  };
  let tail: Vec<&str> = tokens.collect();
  argv_matches(&command.words, head, &tail) || argv_matches(&command.argv, head, &tail)
}

#[cfg(test)]
#[path = "tests/rules_test.rs"]
mod tests;
