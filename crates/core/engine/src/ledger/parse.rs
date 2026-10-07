//! Plan document parsing (`packages/toolu-core/src/ledger/ledger-parse.ts`):
//! the machine-readable steps block and the path helpers the parsers share.
//! Header fields and acceptance-criterion ids are in `ledger::doc`. Pure
//! reads; nothing here writes.

use std::path::{Component, Path, PathBuf};

use toolu_runtime::config::read::MODEL_ALIASES;
use toolu_runtime::json::ordered::Ordered;

use super::jq::{alt, parse_json, to_str};

/// Why a plan does not parse: the exit code (2 for bad `paths`, else 1) and the tagged message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
  /// The exit code bash returned.
  pub exit: u8,
  /// `plan-ledger-parse: …`.
  pub message: String,
}

/// `[[:space:]]` in the C locale.
pub(crate) fn is_c_space(c: char) -> bool {
  matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
}

/// `^<prefix>[[:space:]]*$`.
pub(crate) fn heading(line: &str, prefix: &str) -> bool {
  line
    .strip_prefix(prefix)
    .is_some_and(|rest| rest.chars().all(is_c_space))
}

/// Node's `path.resolve(cwd, path)`: `path` against `cwd`, `.` and `..` folded lexically.
pub fn resolve(cwd: &Path, path: &str) -> PathBuf {
  let mut out = PathBuf::from("/");
  for component in cwd.join(path).components() {
    match component {
      Component::ParentDir => {
        out.pop();
      }
      Component::Normal(name) => out.push(name),
      Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
    }
  }
  out
}

/// bash `[ -f PATH ]`: an existing regular file, symlinks followed.
pub fn is_file(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

/// The file's lines as awk and grep see them, or none when it cannot be read.
pub fn lines(path: &Path) -> Vec<String> {
  let Ok(bytes) = std::fs::read(path) else {
    return Vec::new();
  };
  let text = String::from_utf8_lossy(&bytes);
  let mut out: Vec<String> = text.split('\n').map(str::to_owned).collect();
  if out.last().is_some_and(String::is_empty) {
    out.pop();
  }
  out
}

/// The first ```` ```json ```` block under the steps heading, as `$(awk ...)` captures it.
fn steps_block(path: &Path) -> String {
  let mut captured: Vec<String> = Vec::new();
  let (mut in_steps, mut in_block) = (false, false);
  for line in lines(path) {
    if heading(&line, "## Steps (machine-readable)") {
      in_steps = true;
    } else if in_steps && !in_block && heading(&line, "```json") {
      in_block = true;
    } else if in_block && heading(&line, "```") {
      break;
    } else if in_block {
      captured.push(line);
    }
  }
  captured.join("\n").trim_end_matches('\n').to_owned()
}

fn non_empty_string(value: Option<&Ordered>) -> bool {
  matches!(value, Some(Ordered::String(text)) if !text.is_empty())
}

/// A step object with non-empty string id, title and check.
fn is_step(value: &Ordered) -> bool {
  matches!(value, Ordered::Object(_))
    && ["id", "title", "check"]
      .iter()
      .all(|key| non_empty_string(value.get(key)))
}

/// A field that is present and not null (`(step.x ?? null) !== null`).
fn present<'a>(step: &'a Ordered, key: &str) -> Option<&'a Ordered> {
  step
    .get(key)
    .filter(|value| !matches!(value, Ordered::Null))
}

fn id_of(step: &Ordered) -> String {
  to_str(step.get("id").unwrap_or(&Ordered::Null))
}

/// `id=model` for each step whose `model` is present but not a routable alias.
fn bad_models(steps: &[Ordered]) -> Vec<String> {
  steps
    .iter()
    .filter_map(|step| {
      let model = present(step, "model")?;
      let routable =
        matches!(model, Ordered::String(name) if MODEL_ALIASES.contains(&name.as_str()));
      (!routable).then(|| format!("{}={}", id_of(step), to_str(model)))
    })
    .collect()
}

/// Ids of steps whose `paths` is present but not an array of non-empty strings.
fn bad_paths(steps: &[Ordered]) -> Vec<String> {
  steps
    .iter()
    .filter(|step| {
      present(step, "paths").is_some_and(|paths| match paths {
        Ordered::Array(items) => items.iter().any(|item| !non_empty_string(Some(item))),
        Ordered::Bool(_)
        | Ordered::Number(_)
        | Ordered::String(_)
        | Ordered::Object(_)
        | Ordered::Null => true,
      })
    })
    .map(id_of)
    .collect()
}

/// The optional authored fields backfilled with jq `//` defaults: existing keys
/// keep their place and missing ones are appended.
fn normalize(step: &Ordered) -> Ordered {
  let mut out = step.clone();
  let empty = Ordered::Array(Vec::new());
  for (key, fallback) in [
    ("ac_refs", &empty),
    ("depends_on", &empty),
    ("paths", &empty),
    ("input", &Ordered::Null),
    ("model", &Ordered::Null),
  ] {
    let value = alt(step.get(key).unwrap_or(&Ordered::Null), fallback).clone();
    out.set(key, value);
  }
  out
}

fn fail(exit: u8, message: &str) -> ParseFailure {
  ParseFailure {
    exit,
    message: format!("plan-ledger-parse: {message}"),
  }
}

/// `pl_parse_steps DOC`: the validated, normalized steps. `doc` is resolved
/// against `cwd` but echoed as given. A bad `paths` field fails with exit 2,
/// every other failure with exit 1.
///
/// # Errors
/// [`ParseFailure`] for a missing doc, block or step field, a bad model or bad paths.
pub fn parse_steps(doc: &str, cwd: &Path) -> Result<Vec<Ordered>, ParseFailure> {
  let path = resolve(cwd, doc);
  if !is_file(&path) {
    return Err(fail(1, &format!("plan doc not found: {doc}")));
  }
  let block = steps_block(&path);
  if block.is_empty() {
    return Err(fail(
      1,
      &format!("no '## Steps (machine-readable)' json block in {doc}"),
    ));
  }
  let steps = match parse_json(&block) {
    Some(Ordered::Array(items)) if !items.is_empty() && items.iter().all(is_step) => items,
    _ => {
      return Err(fail(
        1,
        &format!("steps block in {doc} is not a non-empty array of {{id,title,check}} strings"),
      ));
    }
  };
  let models = bad_models(&steps);
  if !models.is_empty() {
    let allowed = MODEL_ALIASES.join(" ");
    return Err(fail(
      1,
      &format!(
        "invalid step model in {doc} ({}); allowed: {allowed}",
        models.join(", ")
      ),
    ));
  }
  let paths = bad_paths(&steps);
  if !paths.is_empty() {
    return Err(fail(
      2,
      &format!(
        "invalid step paths in {doc} ({}); expected an array of non-empty strings",
        paths.join(", ")
      ),
    ));
  }
  Ok(steps.iter().map(normalize).collect())
}

#[cfg(test)]
#[path = "tests/parse_test.rs"]
mod tests;
