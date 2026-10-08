//! Result JSON for `cargo xtask bench`: the shape in `benchmarks/results/README.md`.

use std::path::{Path, PathBuf};

use serde_json::Value;

const REQUIRED: [&str; 15] = [
  "mechanism",
  "tier",
  "method",
  "tokenizer.mode",
  "tokenizer.source",
  "provenance.date",
  "provenance.commit",
  "provenance.pricing_id",
  "provenance.n_runs",
  "baseline.label",
  "baseline.tokens",
  "treatment.label",
  "treatment.tokens",
  "delta.tokens_pct",
  "delta.abs_tokens",
];

/// Write `doc` as `mechanism-tier-date.json` after the mode check, then validate it.
pub(crate) fn write_result(
  doc: &Value,
  modes: &[String],
  out_dir: &Path,
) -> Result<PathBuf, String> {
  let mode = dig(doc, "tokenizer.mode")
    .and_then(Value::as_str)
    .unwrap_or("");
  if !modes.is_empty() && !modes.iter().all(|item| item == mode) {
    return Err("bench: mixed tokenizer modes in one delta; refusing to write".to_owned());
  }
  let mechanism = required_str(doc, "mechanism")?;
  let tier = required_str(doc, "tier")?;
  let date = required_str(doc, "provenance.date")?;
  std::fs::create_dir_all(out_dir)
    .map_err(|err| format!("bench: cannot create {}: {err}", out_dir.display()))?;
  let path = out_dir.join(format!("{mechanism}-{tier}-{date}.json"));
  let text = serde_json::to_string_pretty(doc)
    .map_err(|err| format!("bench: cannot encode result: {err}"))?;
  std::fs::write(&path, format!("{text}\n"))
    .map_err(|err| format!("bench: cannot write {}: {err}", path.display()))?;
  validate(&path)?;
  Ok(path)
}

/// Every required key is present, and `provenance.model` exists even when null.
pub(crate) fn validate(path: &Path) -> Result<(), String> {
  if !path.is_file() {
    return Err(format!("bench: file not found: {}", path.display()));
  }
  let name = path.display().to_string();
  let text =
    std::fs::read_to_string(path).map_err(|err| format!("bench: cannot read {name}: {err}"))?;
  let doc: Value =
    serde_json::from_str(&text).map_err(|err| format!("bench: invalid JSON in {name}: {err}"))?;
  if let Some(problem) = problems(&doc, &name).into_iter().next() {
    return Err(format!("bench: {problem}"));
  }
  Ok(())
}

fn problems(doc: &Value, name: &str) -> Vec<String> {
  if !doc.is_object() {
    return vec![format!("invalid JSON in {name}")];
  }
  let missing = REQUIRED
    .into_iter()
    .filter(|path| !present(doc, path))
    .collect::<Vec<_>>();
  if !missing.is_empty() {
    return vec![format!(
      "{name} missing/empty required keys: {}",
      missing.join(",")
    )];
  }
  let model = doc
    .get("provenance")
    .and_then(Value::as_object)
    .is_some_and(|obj| obj.contains_key("model"));
  if model {
    Vec::new()
  } else {
    vec![format!("{name} missing provenance.model key")]
  }
}

fn present(doc: &Value, path: &str) -> bool {
  match dig(doc, path) {
    None | Some(Value::Null) => false,
    Some(Value::String(text)) => !text.is_empty(),
    Some(_) => true,
  }
}

fn required_str<'a>(doc: &'a Value, path: &str) -> Result<&'a str, String> {
  match dig(doc, path).and_then(Value::as_str) {
    Some(text) if !text.is_empty() => Ok(text),
    _ => Err("bench: result needs mechanism, tier, and provenance.date".to_owned()),
  }
}

fn dig<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
  let mut cur = value;
  for key in path.split('.') {
    cur = cur.as_object()?.get(key)?;
  }
  Some(cur)
}

#[cfg(test)]
#[path = "tests/bench_result_test.rs"]
mod tests;
