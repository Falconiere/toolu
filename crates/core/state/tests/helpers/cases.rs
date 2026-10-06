//! The shared case files, objects kept in document order.

use std::path::Path;

use toolu_runtime::json::ordered::Ordered;

/// The cases of `fixtures/<rel>` whose `kind` is `kind`.
pub(crate) fn cases_of(rel: &str, kind: &str) -> Result<Vec<Ordered>, String> {
  let file = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../../fixtures")
    .join(rel);
  let text = std::fs::read_to_string(&file).map_err(|err| format!("{rel}: {err}"))?;
  let doc = Ordered::parse(&text)?;
  let Some(Ordered::Array(cases)) = doc.get("cases") else {
    return Err(format!("{rel}: no cases array"));
  };
  let wanted = Ordered::String(kind.to_owned());
  Ok(
    cases
      .iter()
      .filter(|case| case.get("kind") == Some(&wanted))
      .cloned()
      .collect(),
  )
}

/// `case[key]`.
pub(crate) fn field<'a>(case: &'a Ordered, key: &str) -> Result<&'a Ordered, String> {
  case
    .get(key)
    .ok_or_else(|| format!("no {key} in {}", case.to_text(false)))
}

/// `case[key]` as a string.
pub(crate) fn text(case: &Ordered, key: &str) -> Result<String, String> {
  if let Ordered::String(text) = field(case, key)? {
    return Ok(text.clone());
  }
  Err(format!("{key} is not a string"))
}
