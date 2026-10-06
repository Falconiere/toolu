//! `cargo xtask check-markdown-cli`: the Markdown that tells an agent what to
//! run must match the `toolu` CLI (#444). Every `toolu …` in a shell-tagged
//! fence or an inline code span is judged against `docs/cli/commands.json`,
//! which the gate's `docs-cli` step proves current; every other fenced command
//! must be a shell builtin, a function, a path, a variable or an allow-listed
//! external command; and a plugin's Markdown may not run a removed surface
//! once its namespace is ported. See `docs/markdown-cli.md`.

mod allow;
mod judge;
mod names;
mod scan;
mod shell;
mod surfaces;
mod words;

use std::fmt;

use regex::Regex;
use serde_json::Value;

use crate::cli_compat::TREE;
use crate::options::Options;
use crate::{Verdict, output};
use allow::Allowlist;
use judge::Origin;
use words::{Command, Lexed, unquote};

/// One problem at one Markdown line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
  file: String,
  line: usize,
  /// The command, flag path or surface the finding is about.
  subject: String,
  problem: String,
}

impl fmt::Display for Finding {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}:{}: `{}`: {}",
      self.file, self.line, self.subject, self.problem
    )
  }
}

/// What every file is judged against.
struct Context {
  tree: Value,
  lists: Vec<Allowlist>,
  patterns: Vec<Regex>,
}

/// Scan every Markdown surface under `--root` and report what drifted.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  let text = std::fs::read_to_string(root.join(TREE))
    .map_err(|err| format!("cannot read {TREE}: {err} — run `cargo xtask docs-cli`"))?;
  let tree = serde_json::from_str(&text).map_err(|err| format!("{TREE} is not JSON: {err}"))?;
  let context = Context {
    tree,
    lists: allow::load(root)?,
    patterns: surfaces::patterns()?,
  };
  let mut found = Vec::new();
  for file in scan::files(root)? {
    let markdown = std::fs::read_to_string(root.join(&file))
      .map_err(|err| format!("cannot read {file}: {err}"))?;
    found.extend(check(&context, &file, &markdown));
  }
  found.sort();
  found.dedup();
  Ok(output::findings(
    "check-markdown-cli",
    &allow::apply(&context.lists, found),
  ))
}

/// Every finding in one Markdown file.
fn check(context: &Context, file: &str, markdown: &str) -> Vec<Finding> {
  let plugin = file
    .strip_prefix("plugins/")
    .and_then(|rest| rest.split('/').next());
  let external = allow::external(&context.lists, file);
  let scanned = scan::scan(markdown);
  let mut found = Vec::new();
  let mut finding = |line: usize, subject: String, problem: String| {
    found.push(Finding {
      file: file.to_owned(),
      line,
      subject,
      problem,
    });
  };
  let lexed: Vec<Lexed> = scanned
    .blocks
    .iter()
    .map(|block| shell::lex(&block.text, block.line))
    .collect();
  // A function one block defines runs in the file's later blocks too.
  let functions: Vec<String> = lexed
    .iter()
    .flat_map(|block| block.functions.iter().cloned())
    .collect();
  for (block, lexed) in scanned.blocks.iter().zip(&lexed) {
    for command in &lexed.commands {
      if let Some((subject, problem)) = fenced(context, command, &functions, &external) {
        finding(command.line, subject, problem);
      }
      let script = plugin.and_then(|_| surfaces::bun_script(&context.patterns, command));
      if let Some((subject, problem)) =
        script.and_then(|reference| removed(context, reference, plugin))
      {
        finding(command.line, subject, problem);
      }
    }
    for (offset, text) in block.text.lines().enumerate() {
      for (subject, problem) in surface_problems(context, text, plugin) {
        finding(block.line + offset, subject, problem);
      }
    }
  }
  for span in &scanned.spans {
    for (subject, problem) in inline(context, &span.text) {
      finding(span.line, subject, problem);
    }
    for (subject, problem) in surface_problems(context, &span.text, plugin) {
      finding(span.line, subject, problem);
    }
  }
  found
}

/// A fenced command: `toolu` against the tree, anything else by name.
fn fenced(
  context: &Context,
  command: &Command,
  functions: &[String],
  external: &std::collections::BTreeSet<String>,
) -> Option<(String, String)> {
  let name = command.words.first()?;
  if unquote(name) == "toolu" {
    let words: Vec<String> = command.words.iter().map(|word| unquote(word)).collect();
    return judge::judge(&context.tree, &words, Origin::Fenced)
      .map(|problem| (command.words.join(" "), problem));
  }
  names::judge_name(name, functions, external).map(|problem| (name.clone(), problem))
}

/// An inline span that runs `toolu`: `toolu` and then a command-like word.
fn inline(context: &Context, text: &str) -> Vec<(String, String)> {
  if !text.starts_with("toolu ") {
    return Vec::new();
  }
  shell::lex(text, 1)
    .commands
    .into_iter()
    .filter(|command| command.words.first().is_some_and(|name| name == "toolu"))
    .filter(|command| command.words.get(1).is_some_and(|word| command_like(word)))
    .filter_map(|command| {
      let words: Vec<String> = command.words.iter().map(|word| unquote(word)).collect();
      judge::judge(&context.tree, &words, Origin::Inline)
        .map(|problem| (command.words.join(" "), problem))
    })
    .collect()
}

/// A lowercase word, a flag, a placeholder or an ellipsis: what follows
/// `toolu` in a command, unlike `toolu PostToolUse failed` or `toolu runtime:`.
fn command_like(word: &str) -> bool {
  let word = unquote(word);
  word.starts_with('-')
    || judge::is_placeholder(&word)
    || word == "…"
    || word == "..."
    || word.chars().next().is_some_and(|c| c.is_ascii_lowercase())
      && word
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The removed-surface paths in one line of code of a plugin's Markdown.
fn surface_problems(context: &Context, text: &str, plugin: Option<&str>) -> Vec<(String, String)> {
  if plugin.is_none() {
    return Vec::new();
  }
  surfaces::references(&context.patterns, text)
    .into_iter()
    .filter_map(|reference| removed(context, reference, plugin))
    .collect()
}

fn removed(
  context: &Context,
  (subject, stem): (String, String),
  plugin: Option<&str>,
) -> Option<(String, String)> {
  surfaces::problem(&context.tree, &stem, plugin?).map(|problem| (subject, problem))
}

#[cfg(test)]
#[path = "tests/markdown_cli_test.rs"]
mod tests;
