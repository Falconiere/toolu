//! Plan and spec header fields and acceptance-criterion ids
//! (`packages/toolu-core/src/ledger/ledger-parse.ts`): `**Key:**` values, the
//! spec's `AC-<n>` ids and the plan's `ac_refs` checked against them. Pure
//! reads; nothing here writes.

use std::cmp::Ordering;
use std::path::Path;

use toolu_runtime::json::ordered::Ordered;

use super::jq::{each_optional, equals, raw, type_name};
use super::parse::{heading, is_c_space, is_file, lines, parse_steps, resolve};

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
#[path = "tests/doc_test.rs"]
mod tests;
