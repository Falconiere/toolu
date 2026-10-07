//! Plan and spec document parsing (`packages/toolu-core/src/ledger/ledger-parse.ts`):
//! the machine-readable steps block, inline-bold header fields and the spec's
//! acceptance-criterion ids. Pure reads; nothing here writes.

use std::cmp::Ordering;
use std::path::{Component, Path, PathBuf};

use toolu_runtime::config::read::MODEL_ALIASES;
use toolu_runtime::json::ordered::Ordered;

use super::jq::{alt, each_optional, equals, parse_json, raw, to_str, type_name};

/// Why a plan does not parse: the exit code (2 for bad `paths`, else 1) and the tagged message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure {
  /// The exit code bash returned.
  pub exit: u8,
  /// `plan-ledger-parse: …`.
  pub message: String,
}

/// `[[:space:]]` in the C locale.
pub fn is_c_space(c: char) -> bool {
  matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
}

/// `^<prefix>[[:space:]]*$`.
fn heading(line: &str, prefix: &str) -> bool {
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

/// Where `**Key:**` starts after white space in `text`: the cut of `docField`'s
/// second replace (`[[:space:]]+\*\*[^*]+:\*\*.*$`).
fn next_field(text: &str) -> Option<usize> {
  let mut run_start = None;
  for (at, c) in text.char_indices() {
    if is_c_space(c) {
      run_start.get_or_insert(at);
      continue;
    }
    if let Some(start) = run_start.take()
      && let Some(rest) = text.get(at..).and_then(|rest| rest.strip_prefix("**"))
      && let Some(star) = rest.find('*')
      && rest.get(star..).is_some_and(|tail| tail.starts_with("**"))
      && rest
        .get(..star)
        .is_some_and(|name| name.chars().count() >= 2 && name.ends_with(':'))
    {
      return Some(start);
    }
  }
  None
}

/// `pl_doc_field DOC FIELD`: the trimmed value of `**FIELD:**` on the first line
/// holding it, cut at the next white-space-preceded `**Key:**`. Empty when the
/// doc or the field is absent.
pub fn doc_field(doc: &Path, field: &str) -> String {
  if field.is_empty() || !is_file(doc) {
    return String::new();
  }
  let key = format!("**{field}:**");
  let all = lines(doc);
  let Some(line) = all.iter().find(|line| line.contains(&key)) else {
    return String::new();
  };
  let after = line
    .rfind(&key)
    .and_then(|at| line.get(at + key.len()..))
    .unwrap_or_default()
    .trim_start_matches(is_c_space);
  let value = next_field(after)
    .and_then(|cut| after.get(..cut))
    .unwrap_or(after);
  value.trim_matches(is_c_space).to_owned()
}

/// The first `**AC-<digits>:**` id on `line`.
fn ac_id(line: &str) -> Option<String> {
  line.match_indices("**AC-").find_map(|(at, _)| {
    let rest = line.get(at + 5..)?;
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    let tail = rest.get(digits..)?;
    (digits > 0 && tail.starts_with(":**"))
      .then(|| format!("AC-{}", rest.get(..digits).unwrap_or_default()))
  })
}

/// `pl_parse_acs SPEC`: `AC-<n>` ids under `## Acceptance criteria`, deduped in document order.
pub fn parse_acs(doc: &Path) -> Vec<String> {
  if !is_file(doc) {
    return Vec::new();
  }
  let mut ids: Vec<String> = Vec::new();
  let mut in_ac = false;
  for line in lines(doc) {
    if heading(&line, "## Acceptance criteria") {
      in_ac = true;
      continue;
    }
    if in_ac && line.starts_with("## ") {
      in_ac = false;
    }
    if let Some(id) = ac_id(&line).filter(|_| in_ac)
      && !ids.contains(&id)
    {
      ids.push(id);
    }
  }
  ids
}

/// bash `tr '[:upper:]' '[:lower:]'` then `""|none`: the spec-less marker.
pub fn is_specless(spec: &str) -> bool {
  spec.is_empty() || spec.eq_ignore_ascii_case("none")
}

fn rank(value: &Ordered) -> u8 {
  match type_name(value) {
    "null" => 0,
    "boolean" => 1,
    "number" => 2,
    "string" => 3,
    "array" => 4,
    _ => 5,
  }
}

/// jq's order between two values, as `ledger-parse.ts` approximates it.
fn jq_order(a: &Ordered, b: &Ordered) -> Ordering {
  rank(a).cmp(&rank(b)).then_with(|| match (a, b) {
    (Ordered::Number(x), Ordered::Number(y)) => {
      let (x, y) = (x.as_f64().unwrap_or(0.0), y.as_f64().unwrap_or(0.0));
      x.partial_cmp(&y).unwrap_or(Ordering::Equal)
    }
    (Ordered::String(x), Ordered::String(y)) => x.cmp(y),
    (Ordered::Bool(x), Ordered::Bool(y)) => x.cmp(y),
    _ => a.to_text(false).cmp(&b.to_text(false)),
  })
}

/// The result of `pl_check_ac_refs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcRefs {
  /// No dangling reference, and the plan parsed.
  pub ok: bool,
  /// The referenced ids the spec does not declare, in byte order.
  pub dangling: Vec<String>,
  /// The parse failure, when the plan does not parse.
  pub message: Option<String>,
}

/// `pl_check_ac_refs PLAN [SPEC]`: every `ac_refs` id the spec does not
/// declare. A spec-less plan passes; a plan that does not parse fails with the
/// parse message.
pub fn check_ac_refs(plan: &str, spec: &str, cwd: &Path) -> AcRefs {
  let pass = AcRefs {
    ok: true,
    dangling: Vec::new(),
    message: None,
  };
  if is_specless(spec) {
    return pass;
  }
  let steps = match parse_steps(plan, cwd) {
    Ok(steps) => steps,
    Err(failure) => {
      return AcRefs {
        ok: false,
        dangling: Vec::new(),
        message: Some(failure.message),
      };
    }
  };
  let mut refs: Vec<&Ordered> = steps
    .iter()
    .flat_map(|step| each_optional(step.get("ac_refs").unwrap_or(&Ordered::Null)))
    .collect();
  refs.sort_by(|a, b| jq_order(a, b));
  refs.dedup_by(|a, b| equals(a, b));
  let text = refs
    .iter()
    .map(|value| raw(value))
    .collect::<Vec<_>>()
    .join("\n");
  let text = text.trim_end_matches('\n');
  if text.is_empty() {
    return pass;
  }
  let declared = parse_acs(&resolve(cwd, spec));
  let mut dangling: Vec<String> = text
    .split('\n')
    .filter(|id| !declared.iter().any(|known| known == id))
    .map(str::to_owned)
    .collect();
  dangling.sort();
  dangling.dedup();
  AcRefs {
    ok: dangling.is_empty(),
    dangling,
    message: None,
  }
}

#[cfg(test)]
#[path = "tests/parse_test.rs"]
mod tests;
