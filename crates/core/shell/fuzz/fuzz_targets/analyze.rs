//! Fuzz `toolu_shell::analyze` on arbitrary input, then every helper a gate
//! calls on its result: tree-sitter's parser and error recovery, the walk, the
//! `bash -c`/`eval` re-parse, and the git and write readers.
#![no_main]

use toolu_shell::git::{commit_messages, git_invocation, push_targets, runs_git_subcommand};
use toolu_shell::rules::matches_rule;
use toolu_shell::writes::write_targets;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
  let Ok(source) = std::str::from_utf8(data) else {
    return;
  };
  let analysis = toolu_shell::analyze(source);
  std::hint::black_box(runs_git_subcommand(&analysis, "push"));
  std::hint::black_box(push_targets(&analysis).len());
  std::hint::black_box(write_targets(&analysis).len());
  for command in &analysis.commands {
    std::hint::black_box(matches_rule(command, "node -e"));
    if let Some(git) = git_invocation(command) {
      std::hint::black_box(commit_messages(&git).len());
    }
  }
});
