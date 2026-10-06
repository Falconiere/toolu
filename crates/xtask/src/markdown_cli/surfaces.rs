//! Removed surfaces: a plugin's Markdown that still runs a Bun bundle
//! (`hooks/dist/<stem>.js`), a TypeScript script (`scripts/<stem>.ts`, or any
//! `.ts`/`.js` file `bun` runs) or a stable script (`jev.sh`, …) fails once
//! the namespace that replaces it is ported: present in the tree, with no
//! `placeholder` verb.

use std::path::Path;

use regex::Regex;
use serde_json::Value;

use crate::command_tree::list;

use super::words::{Command, unquote};

/// Paths of removed surfaces in code text: the reference is group `path`, the
/// file's stem group `stem`. A stable script name stands alone or after `/`.
const PATTERNS: &[&str] = &[
  r"(?P<path>hooks/dist/(?P<stem>[A-Za-z0-9_.-]+)\.js)\b",
  r"(?P<path>scripts/(?:[A-Za-z0-9_.-]+/)*(?P<stem>[A-Za-z0-9_.-]+)\.ts)\b",
  r#"(?:^|[/\s"'=`(|;])(?P<path>(?P<stem>jev|write-state|search|statusline)\.sh)\b"#,
];

/// Stems whose namespace is not the one their plugin owns: exact, or a
/// prefix when the entry ends in `-`.
const STEMS: &[(&str, &str)] = &[
  ("plan-ledger", "ledger"),
  ("verdict", "ledger"),
  ("jev", "jev"),
  ("write-state", "review"),
  ("babysit-", "babysit"),
  ("debug-", "debug"),
  ("setup", "setup"),
  ("statusline", "statusline"),
  ("status", "statusline"),
  ("search", "ast-grep"),
  ("ast-grep", "ast-grep"),
  ("byte-savings", "ast-grep"),
];

/// The compiled [`PATTERNS`].
pub(crate) fn patterns() -> Result<Vec<Regex>, String> {
  PATTERNS
    .iter()
    .map(|pattern| Regex::new(pattern).map_err(|err| format!("surface pattern {pattern}: {err}")))
    .collect()
}

/// Each removed-surface path in `code` with its stem.
pub(crate) fn references(patterns: &[Regex], code: &str) -> Vec<(String, String)> {
  let mut found = Vec::new();
  for pattern in patterns {
    for captures in pattern.captures_iter(code) {
      if let (Some(all), Some(stem)) = (captures.name("path"), captures.name("stem")) {
        found.push((all.as_str().to_owned(), stem.as_str().to_owned()));
      }
    }
  }
  found
}

/// The script a `bun` command runs, when it is a `.ts`/`.js` file the path
/// patterns do not already catch: `bun "$S/launch-issue.ts"`.
pub(crate) fn bun_script(patterns: &[Regex], command: &Command) -> Option<(String, String)> {
  let mut words = command.words.iter().map(|word| unquote(word));
  let name = words.next()?;
  if !(name == "bun" || name.ends_with("/bun") || name.contains("BUN")) {
    return None;
  }
  // `bun [run] <file>`: the file is the first operand after `run`; `bun test`,
  // `bun build` and `bun run <script name>` run no file of their own.
  let mut first = operand(&mut words)?;
  if first == "run" {
    first = operand(&mut words)?;
  }
  let script = Some(first).filter(|word| {
    Path::new(word)
      .extension()
      .is_some_and(|ext| ext.eq_ignore_ascii_case("ts") || ext.eq_ignore_ascii_case("js"))
  })?;
  let stem = Path::new(&script)
    .file_stem()?
    .to_string_lossy()
    .into_owned();
  let caught = patterns.iter().any(|pattern| pattern.is_match(&script));
  (!caught).then_some((script, stem))
}

/// `bun` flags whose value is the next word.
const VALUE_FLAGS: &[&str] = &["--cwd", "--env-file", "--config", "-c", "--preload", "-r"];

/// The next word that is not a flag or a flag's value.
fn operand(words: &mut impl Iterator<Item = String>) -> Option<String> {
  loop {
    let word = words.next()?;
    if VALUE_FLAGS.contains(&word.as_str()) {
      words.next();
    } else if !word.starts_with('-') {
      return Some(word);
    }
  }
}

/// Why the reference to `stem` in plugin `plugin`'s Markdown is removed, if it is.
pub(crate) fn problem(tree: &Value, stem: &str, plugin: &str) -> Option<String> {
  let namespaces = namespaces(tree, stem, plugin);
  let ported = !namespaces.is_empty() && namespaces.iter().all(|name| is_ported(tree, name));
  ported.then(|| {
    let shown: Vec<String> = namespaces
      .iter()
      .map(|name| format!("`toolu {name}`"))
      .collect();
    format!(
      "a removed surface: {} is ported; run it instead",
      shown.join(", ")
    )
  })
}

/// The namespace that replaces `stem`: from [`STEMS`], else every namespace
/// the plugin owns.
fn namespaces(tree: &Value, stem: &str, plugin: &str) -> Vec<String> {
  let matches = |entry: &str| match entry.strip_suffix('-') {
    Some(_) => stem.starts_with(entry),
    None => stem == entry,
  };
  if let Some((_, namespace)) = STEMS.iter().find(|(entry, _)| matches(entry)) {
    return vec![(*namespace).to_owned()];
  }
  commands(tree)
    .iter()
    .filter(|command| command.get("owner").and_then(Value::as_str) == Some(plugin))
    .filter_map(|command| command.get("name").and_then(Value::as_str))
    .map(str::to_owned)
    .collect()
}

fn is_ported(tree: &Value, namespace: &str) -> bool {
  commands(tree)
    .iter()
    .find(|command| command.get("name").and_then(Value::as_str) == Some(namespace))
    .is_some_and(|command| {
      !commands(command)
        .iter()
        .any(|verb| verb.get("placeholder") == Some(&Value::Bool(true)))
    })
}

fn commands(node: &Value) -> &[Value] {
  list(node, "commands")
}

#[cfg(test)]
#[path = "tests/surfaces_test.rs"]
mod tests;
