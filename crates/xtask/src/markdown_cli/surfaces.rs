//! Removed surfaces: a plugin's Markdown that still runs a Bun bundle
//! (`hooks/dist/<stem>.js`), a TypeScript script (`scripts/<stem>.ts`, or any
//! `.ts`/`.js` file `bun` runs) or a stable script (`jev.sh`, …) fails once
//! the namespace that replaces it is ported: present in the tree, with no
//! `placeholder` verb.

use regex::Regex;
use serde_json::Value;

use super::words::{Command, unquote};

/// Paths of removed surfaces in code text, with the stem in group 1.
const PATTERNS: &[&str] = &[
  r"hooks/dist/([A-Za-z0-9_.-]+)\.js\b",
  r"scripts/(?:[A-Za-z0-9_.-]+/)*([A-Za-z0-9_.-]+)\.ts\b",
  r"\b(jev|write-state|search|statusline)\.sh\b",
];

/// Stems whose namespace is not the one their plugin owns, by prefix.
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
      if let (Some(all), Some(stem)) = (captures.get(0), captures.get(1)) {
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
  let script = words.find(|word| !word.starts_with('-'))?;
  let file = script.rsplit('/').next()?;
  let stem = file
    .strip_suffix(".ts")
    .or_else(|| file.strip_suffix(".js"))?;
  let caught = patterns.iter().any(|pattern| pattern.is_match(&script));
  (!caught).then(|| (script.clone(), stem.to_owned()))
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
  if let Some((_, namespace)) = STEMS.iter().find(|(prefix, _)| stem.starts_with(prefix)) {
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
  node
    .get("commands")
    .and_then(Value::as_array)
    .map_or(&[], Vec::as_slice)
}

#[cfg(test)]
#[path = "tests/surfaces_test.rs"]
mod tests;
