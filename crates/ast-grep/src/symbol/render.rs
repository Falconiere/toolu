//! Print a lookup within its caps, as text or one JSON document.

use serde_json::{Value, json};

use super::{Definition, Found, Reference};

const MAX_DEFINITIONS: usize = 10;
const MAX_REFERENCES: usize = 40;
const MAX_PER_FILE: usize = 8;
const LINE_CHARS: usize = 120;
const NOTE: &str = "note: ast-grep not found on PATH; definitions and enclosing items are skipped (install: brew install ast-grep)";

/// The references shown under each file, in order, and how many each file hides.
type Shown<'a> = Vec<(&'a str, Vec<&'a Reference>, usize)>;

/// `text` cut to [`LINE_CHARS`] characters, ending in `...` when cut.
fn cut(text: &str) -> String {
  if text.chars().count() <= LINE_CHARS {
    return text.to_owned();
  }
  let mut short: String = text.chars().take(LINE_CHARS - 3).collect();
  short.push_str("...");
  short
}

fn plural(count: usize, noun: &str) -> String {
  let suffix = if count == 1 { "" } else { "s" };
  format!("{count} {noun}{suffix}")
}

/// Group references by file, at most [`MAX_PER_FILE`] each and
/// [`MAX_REFERENCES`] in all; files past the total cap are left out.
fn shown(references: &[Reference]) -> Shown<'_> {
  let mut groups: Shown<'_> = Vec::new();
  let mut budget = MAX_REFERENCES;
  for one in references {
    match groups.last_mut() {
      Some((path, _, _)) if *path == one.path => {}
      _ if budget == 0 => break,
      _ => groups.push((one.path.as_str(), Vec::new(), 0)),
    }
    let Some((_, refs, hidden)) = groups.last_mut() else {
      break;
    };
    if budget > 0 && refs.len() < MAX_PER_FILE {
      refs.push(one);
      budget -= 1;
    } else {
      *hidden += 1;
    }
  }
  groups
}

/// Whether any cap dropped output.
fn truncated(found: &Found, groups: &Shown<'_>) -> bool {
  let printed: usize = groups.iter().map(|(_, refs, _)| refs.len()).sum();
  found.partial
    || found.unoutlined > 0
    || found.definitions.len() > MAX_DEFINITIONS
    || printed < found.references.len()
}

fn reference_line(one: &Reference) -> String {
  match &one.within {
    Some((item, start, end)) => format!("  {} in {item} {start}-{end}: {}", one.line, one.text),
    None => format!("  {}: {}", one.line, one.text),
  }
}

fn text(found: &Found, groups: &Shown<'_>) -> String {
  let partial = if found.partial {
    " (grep output capped at 256 KiB)"
  } else {
    ""
  };
  let mut out = vec![format!(
    "{}: {}, {} in {}{partial}",
    found.name,
    plural(found.definitions.len(), "definition"),
    plural(found.references.len(), "reference"),
    plural(found.files, "file"),
  )];
  out.extend(
    found
      .definitions
      .iter()
      .take(MAX_DEFINITIONS)
      .map(|def: &Definition| {
        format!(
          "def {}:{}-{} {}",
          def.path, def.start, def.end, def.signature
        )
      }),
  );
  for (path, refs, hidden) in groups {
    out.push((*path).to_owned());
    out.extend(refs.iter().map(|one| reference_line(one)));
    if *hidden > 0 {
      out.push(format!("  ... {hidden} more in this file"));
    }
  }
  if found.missing {
    out.push(NOTE.to_owned());
  }
  if truncated(found, groups) {
    out.push(format!(
      "Narrow: toolu ast-grep symbol {} <path>",
      found.name
    ));
  }
  out
    .iter()
    .map(|line| cut(line))
    .collect::<Vec<_>>()
    .join("\n")
}

fn json(found: &Found, groups: &Shown<'_>) -> Value {
  let definitions: Vec<Value> = found
    .definitions
    .iter()
    .take(MAX_DEFINITIONS)
    .map(|def| {
      json!({"path": def.path, "line": def.line, "start": def.start, "end": def.end, "signature": def.signature})
    })
    .collect();
  let references: Vec<Value> = groups
    .iter()
    .flat_map(|(_, refs, _)| refs.iter())
    .map(|one| {
      let within = one
        .within
        .as_ref()
        .map(|(name, start, end)| json!({"name": name, "start": start, "end": end}));
      json!({"path": one.path, "line": one.line, "text": one.text, "in": within})
    })
    .collect();
  json!({
    "name": found.name,
    "definitions": definitions,
    "references": references,
    "totals": {"definitions": found.definitions.len(), "references": found.references.len(), "files": found.files},
    "partial": found.partial,
    "truncated": truncated(found, groups),
    "note": found.missing.then_some(NOTE.trim_start_matches("note: ")),
  })
}

/// Print `found` within its caps.
pub(super) fn render(found: &Found, json_output: bool) -> String {
  let groups = shown(&found.references);
  if json_output {
    json(found, &groups).to_string()
  } else {
    text(found, &groups)
  }
}

#[cfg(test)]
#[path = "tests/render_test.rs"]
mod tests;
