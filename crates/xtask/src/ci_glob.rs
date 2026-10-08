//! Glob translation for `.github/ci-paths.json`.

use regex::Regex;

const SEMVER: &str = r"\^?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?";

/// One glob compiled to a full-path expression.
pub(crate) struct Glob {
  pub(crate) source: String,
  regex: Regex,
}

/// Compile one glob, or name the data-file path that held it.
pub(crate) fn compile(path: &str, source: String) -> Result<Glob, String> {
  let pattern = glob_pattern(&source);
  let regex =
    Regex::new(&pattern).map_err(|err| format!("{path}: glob {source} is invalid: {err}"))?;
  Ok(Glob { source, regex })
}

impl Glob {
  /// Whether this glob matches `path`.
  pub(crate) fn hits(&self, path: &str) -> bool {
    self.regex.is_match(path)
  }
}

/// Whether any compiled glob matches `path`.
pub(crate) fn matches(globs: &[Glob], path: &str) -> bool {
  globs.iter().any(|glob| glob.hits(path))
}

/// A line that changes only a version field named by one of `keys`.
pub(crate) fn version_pattern(keys: &[String]) -> Result<Regex, String> {
  let escaped = keys
    .iter()
    .map(|key| regex::escape(key))
    .collect::<Vec<_>>()
    .join("|");
  let json = format!(r#""(?:{escaped})"\s*:\s*"{SEMVER}"\s*,?"#);
  let toml = format!(r#"(?:{escaped})\s*=\s*"{SEMVER}""#);
  Regex::new(&format!(r"^[+-]\s*(?:{json}|{toml})\s*$"))
    .map_err(|err| format!("version pattern: {err}"))
}

/// `**` matches across directories, including the empty match before a slash.
pub(crate) fn glob_pattern(glob: &str) -> String {
  let mut pattern = String::from("^");
  let mut rest = glob;
  while !rest.is_empty() {
    if let Some(next) = rest.strip_prefix("**/") {
      pattern.push_str("(?:.*/)?");
      rest = next;
    } else if let Some(next) = rest.strip_prefix("**") {
      pattern.push_str(".*");
      rest = next;
    } else if let Some(next) = rest.strip_prefix('*') {
      pattern.push_str("[^/]*");
      rest = next;
    } else if let Some(next) = rest.strip_prefix('?') {
      pattern.push_str("[^/]");
      rest = next;
    } else {
      rest = push_literal(&mut pattern, rest);
    }
  }
  pattern.push('$');
  pattern
}

fn push_literal<'a>(pattern: &mut String, rest: &'a str) -> &'a str {
  let mut chars = rest.chars();
  let Some(ch) = chars.next() else {
    return rest;
  };
  pattern.push_str(&regex::escape(&ch.to_string()));
  chars.as_str()
}

#[cfg(test)]
#[path = "tests/ci_glob_test.rs"]
mod tests;
