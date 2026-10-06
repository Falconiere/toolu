//! The name of a fenced command that is not `toolu`: a shell builtin or
//! keyword, a function the block defines, a path or a variable passes on its
//! own; anything else must be on the external allow-list.

use std::collections::BTreeSet;

use super::words::unquote;

/// Shell builtins and keywords a command may start with.
const BUILTINS: &[&str] = &[
  ".", ":", "[", "[[", "alias", "bg", "break", "builtin", "cd", "command", "continue", "declare",
  "echo", "eval", "exec", "exit", "export", "false", "fg", "getopts", "hash", "jobs", "kill",
  "let", "local", "popd", "printf", "pushd", "pwd", "read", "readonly", "return", "set", "shift",
  "source", "test", "trap", "true", "type", "typeset", "ulimit", "umask", "unalias", "unset",
  "wait",
];

/// Why `name` may not be run, if it may not.
pub(crate) fn judge_name(
  name: &str,
  functions: &[String],
  external: &BTreeSet<String>,
) -> Option<String> {
  let bare = unquote(name);
  let passes = bare.contains('$')
    || bare.contains('/')
    || BUILTINS.contains(&bare.as_str())
    || functions.contains(&bare)
    || external.contains(&bare);
  (!passes).then(|| {
    "not `toolu`, a shell builtin, a function the block defines or a command on the external \
     allow-list (`external` in tooling/conventions/markdown-cli.json or tooling/conventions/markdown-cli/<plugin>.json)"
      .to_owned()
  })
}

#[cfg(test)]
#[path = "tests/names_test.rs"]
mod tests;
