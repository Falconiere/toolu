//! Rule 23: no committed secret in any file under `crates/`.

use regex::Regex;

use super::{Context, Finding};

/// Lines that match a secret pattern of `rules.json`.
pub(super) fn check(ctx: &Context<'_>) -> Result<Vec<Finding>, String> {
  let patterns = ctx
    .rules
    .secrets
    .iter()
    .map(|secret| {
      Regex::new(&secret.regex)
        .map(|regex| (secret.id.as_str(), regex))
        .map_err(|err| format!("rules.json secret {}: {err}", secret.id))
    })
    .collect::<Result<Vec<_>, _>>()?;
  let mut found = Vec::new();
  for file in ctx
    .workspace
    .files
    .iter()
    .filter(|file| file.starts_with("crates"))
  {
    let Ok(text) = std::fs::read_to_string(ctx.workspace.root.join(file)) else {
      continue;
    };
    let path = file.to_string_lossy().replace('\\', "/");
    for (index, line) in text.lines().enumerate() {
      if let Some((id, _)) = patterns.iter().find(|(_, regex)| regex.is_match(line)) {
        found.push(Finding::new(
          "secrets",
          &path,
          index + 1,
          format!("looks like a committed secret ({id}) — remove it and rotate it"),
        ));
      }
    }
  }
  Ok(found)
}

#[cfg(test)]
#[path = "tests/secrets_test.rs"]
mod tests;
