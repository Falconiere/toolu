//! The gate file's strict v1 schema (`GateFileSchema` in `state-schema.ts`):
//! `passing` (`status`, `source`, `updatedAt`) or `failing` (`status`,
//! `reason`, `source`, `file`, `violations`, optional `entries`, `updatedAt`),
//! each closed, with an optional `version` that must be 1. The validator walks
//! keys in zod's shape order and names the first offending one as
//! `<path>: <message>`, unknown keys last, as zod reports them.

use toolu_runtime::json::ordered::Ordered;

use crate::js_order::js_order;

/// The implicit format version; an explicit `"version": 1` is accepted.
pub const GATE_FILE_VERSION: u8 = 1;
/// The entry key of a failure that is about no file.
pub const GLOBAL_GATE_KEY: &str = "__global__";

const PASSING: [&str; 4] = ["version", "status", "source", "updatedAt"];
const FAILING: [&str; 8] = [
  "version",
  "status",
  "reason",
  "source",
  "file",
  "violations",
  "entries",
  "updatedAt",
];
const ENTRY: [&str; 4] = ["source", "reason", "violations", "updatedAt"];

/// One failing file's slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateEntry {
  /// The gate that recorded it.
  pub source: String,
  /// Why it fails.
  pub reason: String,
  /// Its violations, newline-terminated lines.
  pub violations: String,
  /// When it was recorded (`isoSeconds`).
  pub updated_at: String,
}

/// A gate file document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateFile {
  /// Nothing fails.
  Passing {
    /// The gate that last cleared it.
    source: String,
    /// When.
    updated_at: String,
  },
  /// Something fails; the top level mirrors the latest entry.
  Failing {
    /// The latest failure's reason.
    reason: String,
    /// The latest failure's gate.
    source: String,
    /// The latest failure's file.
    file: String,
    /// Every open violation, oldest first.
    violations: String,
    /// The slots, in JavaScript object order; `None` in a legacy single-slot file.
    entries: Option<Vec<(String, GateEntry)>>,
    /// When.
    updated_at: String,
  },
}

fn text(value: &str) -> Ordered {
  Ordered::String(value.to_owned())
}

impl GateEntry {
  fn to_ordered(&self) -> Ordered {
    Ordered::Object(vec![
      ("source".to_owned(), text(&self.source)),
      ("reason".to_owned(), text(&self.reason)),
      ("violations".to_owned(), text(&self.violations)),
      ("updatedAt".to_owned(), text(&self.updated_at)),
    ])
  }
}

impl GateFile {
  /// The document with the TypeScript writers' key order.
  pub fn to_ordered(&self) -> Ordered {
    match self {
      GateFile::Passing { source, updated_at } => Ordered::Object(vec![
        ("status".to_owned(), text("passing")),
        ("source".to_owned(), text(source)),
        ("updatedAt".to_owned(), text(updated_at)),
      ]),
      GateFile::Failing {
        reason,
        source,
        file,
        violations,
        entries,
        updated_at,
      } => {
        let mut fields = vec![
          ("status".to_owned(), text("failing")),
          ("reason".to_owned(), text(reason)),
          ("source".to_owned(), text(source)),
          ("file".to_owned(), text(file)),
          ("violations".to_owned(), text(violations)),
        ];
        if let Some(entries) = entries {
          let slots = entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.to_ordered()));
          fields.push(("entries".to_owned(), Ordered::Object(slots.collect())));
        }
        fields.push(("updatedAt".to_owned(), text(updated_at)));
        Ordered::Object(fields)
      }
    }
  }
}

/// `GateFileSchema.safeParse(value)`: the document, or its first issue.
///
/// # Errors
/// `<path>: <message>`, with `(root)` for the document itself.
pub fn validate_gate_file(value: &Ordered) -> Result<GateFile, String> {
  if !matches!(value, Ordered::Object(_)) {
    return Err(issue("", &expected("object", Some(value))));
  }
  match text_of(value.get("status")).unwrap_or_default() {
    "passing" => {
      version(value)?;
      let (source, updated_at) = (
        string(value, "", "source")?,
        string(value, "", "updatedAt")?,
      );
      closed(value, "", &PASSING)?;
      Ok(GateFile::Passing { source, updated_at })
    }
    "failing" => failing(value),
    _ => Err(issue("status", "Invalid input")),
  }
}

fn failing(value: &Ordered) -> Result<GateFile, String> {
  version(value)?;
  let reason = string(value, "", "reason")?;
  let source = string(value, "", "source")?;
  let file = string(value, "", "file")?;
  let violations = string(value, "", "violations")?;
  let entries = entries(value)?;
  let updated_at = string(value, "", "updatedAt")?;
  closed(value, "", &FAILING)?;
  Ok(GateFile::Failing {
    reason,
    source,
    file,
    violations,
    entries,
    updated_at,
  })
}

fn entries(value: &Ordered) -> Result<Option<Vec<(String, GateEntry)>>, String> {
  let Some(slots) = value.get("entries") else {
    return Ok(None);
  };
  let Ordered::Object(slots) = slots else {
    return Err(issue("entries", &expected("record", Some(slots))));
  };
  let mut read = Vec::new();
  for (key, slot) in js_order(slots.clone()) {
    let at = format!("entries.{key}");
    if !matches!(slot, Ordered::Object(_)) {
      return Err(issue(&at, &expected("object", Some(&slot))));
    }
    let entry = GateEntry {
      source: string(&slot, &at, "source")?,
      reason: string(&slot, &at, "reason")?,
      violations: string(&slot, &at, "violations")?,
      updated_at: string(&slot, &at, "updatedAt")?,
    };
    closed(&slot, &at, &ENTRY)?;
    read.push((key, entry));
  }
  Ok(Some(read))
}

fn text_of(value: Option<&Ordered>) -> Option<&str> {
  if let Some(Ordered::String(text)) = value {
    Some(text)
  } else {
    None
  }
}

/// zod's name for what `value` is.
fn kind(value: Option<&Ordered>) -> &'static str {
  match value {
    None => "undefined",
    Some(Ordered::Null) => "null",
    Some(Ordered::Bool(_)) => "boolean",
    Some(Ordered::Number(_)) => "number",
    Some(Ordered::String(_)) => "string",
    Some(Ordered::Array(_)) => "array",
    Some(Ordered::Object(_)) => "object",
  }
}

fn expected(what: &str, value: Option<&Ordered>) -> String {
  format!("Invalid input: expected {what}, received {}", kind(value))
}

fn issue(path: &str, message: &str) -> String {
  let path = if path.is_empty() { "(root)" } else { path };
  format!("{path}: {message}")
}

fn join(at: &str, key: &str) -> String {
  if at.is_empty() {
    key.to_owned()
  } else {
    format!("{at}.{key}")
  }
}

fn string(object: &Ordered, at: &str, key: &str) -> Result<String, String> {
  let value = object.get(key);
  let text = text_of(value).map(str::to_owned);
  text.ok_or_else(|| issue(&join(at, key), &expected("string", value)))
}

/// `z.literal(1).optional()`.
fn version(object: &Ordered) -> Result<(), String> {
  let Some(value) = object.get("version") else {
    return Ok(());
  };
  let one = if let Ordered::Number(number) = value {
    number.as_f64()
  } else {
    None
  };
  if one.is_some_and(|one| (one - f64::from(GATE_FILE_VERSION)).abs() < f64::EPSILON) {
    Ok(())
  } else {
    Err(issue("version", "Invalid input: expected 1"))
  }
}

/// A strict object's unknown keys, in document order.
fn closed(object: &Ordered, at: &str, known: &[&str]) -> Result<(), String> {
  let Ordered::Object(fields) = object else {
    return Ok(());
  };
  let unknown: Vec<String> = fields
    .iter()
    .filter(|(key, _)| !known.contains(&key.as_str()))
    .map(|(key, _)| format!("\"{key}\""))
    .collect();
  match unknown.as_slice() {
    [] => Ok(()),
    [one] => Err(issue(at, &format!("Unrecognized key: {one}"))),
    many => Err(issue(
      at,
      &format!("Unrecognized keys: {}", many.join(", ")),
    )),
  }
}

#[cfg(test)]
#[path = "tests/gate_schema_test.rs"]
mod tests;
