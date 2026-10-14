//! Validate the committed live host-lifecycle evidence of issue #376
//! (`probe.ts`) and summarize it with every gap kept.

use std::collections::BTreeMap;

use regex::Regex;
use serde_json::{Map, Value};

/// The committed evidence, the default of `toolu epic probe --check-evidence`.
pub(crate) const COMMITTED: &str = include_str!("../../../docs/toolu/evidence/epic-hosts-v1.json");

const HOSTS: [&str; 3] = ["claude", "codex", "cursor"];
const CAPABILITIES: [&str; 6] = [
  "start",
  "tool_task",
  "resume",
  "background_work",
  "cancel_replace",
  "teardown",
];
const OUTCOMES: [&str; 4] = ["verified", "failed", "unsupported", "unverified"];

type Checked<T> = Result<T, String>;

fn object<'a>(value: &'a Value, path: &str) -> Checked<&'a Map<String, Value>> {
  value
    .as_object()
    .ok_or_else(|| format!("{path}: expected object"))
}

fn exact_keys(value: &Map<String, Value>, expected: &[&str], path: &str) -> Checked<()> {
  let mut actual: Vec<&str> = value.keys().map(String::as_str).collect();
  let mut wanted = expected.to_vec();
  actual.sort_unstable();
  wanted.sort_unstable();
  if actual == wanted {
    Ok(())
  } else {
    Err(format!(
      "{path}: expected keys {}; got {}",
      wanted.join(", "),
      actual.join(", ")
    ))
  }
}

fn text<'a>(value: Option<&'a Value>, path: &str) -> Checked<&'a str> {
  value
    .and_then(Value::as_str)
    .filter(|text| !text.trim().is_empty())
    .ok_or_else(|| format!("{path}: expected non-empty string"))
}

fn strings(value: Option<&Value>, path: &str) -> Checked<usize> {
  let items = value
    .and_then(Value::as_array)
    .ok_or_else(|| format!("{path}: expected string array"))?;
  for (index, item) in items.iter().enumerate() {
    text(Some(item), &format!("{path}[{index}]"))?;
  }
  Ok(items.len())
}

fn timestamp(value: Option<&Value>, path: &str) -> Checked<()> {
  let stamp = text(value, path)?;
  let shape = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$")
    .map_err(|err| err.to_string())?;
  if !shape.is_match(stamp) {
    return Err(format!("{path}: expected UTC ISO-8601 timestamp"));
  }
  toolu_state::time::parse_iso(stamp)
    .map(|_| ())
    .ok_or_else(|| format!("{path}: invalid timestamp"))
}

fn native_error(value: Option<&Value>, path: &str) -> Checked<bool> {
  match value {
    Some(Value::Null) => Ok(false),
    Some(value) => {
      let data = object(value, path)?;
      exact_keys(data, &["code", "message", "exitCode"], path)?;
      text(data.get("code"), &format!("{path}.code"))?;
      text(data.get("message"), &format!("{path}.message"))?;
      if !matches!(data.get("exitCode"), Some(Value::Null))
        && !data
          .get("exitCode")
          .is_some_and(|code| code.is_i64() || code.is_u64())
      {
        return Err(format!("{path}.exitCode: expected integer or null"));
      }
      Ok(true)
    }
    None => Err(format!("{path}: expected object")),
  }
}

fn capability(value: Option<&Value>, path: &str) -> Checked<String> {
  let data = object(value.unwrap_or(&Value::Null), path)?;
  exact_keys(
    data,
    &[
      "outcome",
      "summary",
      "observedAt",
      "command",
      "evidence",
      "nativeError",
    ],
    path,
  )?;
  let outcome = text(data.get("outcome"), &format!("{path}.outcome"))?;
  if !OUTCOMES.contains(&outcome) {
    return Err(format!(
      "{path}.outcome: expected one of {}",
      OUTCOMES.join(", ")
    ));
  }
  let evidence = strings(data.get("evidence"), &format!("{path}.evidence"))?;
  let failure = native_error(data.get("nativeError"), &format!("{path}.nativeError"))?;
  if outcome == "verified" && evidence == 0 {
    return Err(format!(
      "{path}.evidence: verified capability requires concrete evidence"
    ));
  }
  if outcome == "failed" && !failure {
    return Err(format!(
      "{path}.nativeError: failed capability requires the native failure"
    ));
  }
  text(data.get("summary"), &format!("{path}.summary"))?;
  timestamp(data.get("observedAt"), &format!("{path}.observedAt"))?;
  strings(data.get("command"), &format!("{path}.command"))?;
  Ok(outcome.to_owned())
}

fn host(value: Option<&Value>, path: &str) -> Checked<BTreeMap<String, String>> {
  let data = object(value.unwrap_or(&Value::Null), path)?;
  exact_keys(
    data,
    &[
      "binary",
      "version",
      "versionCommand",
      "profileIsolation",
      "capabilities",
    ],
    path,
  )?;
  if !matches!(data.get("version"), Some(Value::Null)) {
    text(data.get("version"), &format!("{path}.version"))
      .map_err(|_err| format!("{path}.version: expected non-empty string or null"))?;
  }
  let capabilities = object(
    data.get("capabilities").unwrap_or(&Value::Null),
    &format!("{path}.capabilities"),
  )?;
  exact_keys(capabilities, &CAPABILITIES, &format!("{path}.capabilities"))?;
  let mut outcomes = BTreeMap::new();
  for name in CAPABILITIES {
    outcomes.insert(
      name.to_owned(),
      capability(
        capabilities.get(name),
        &format!("{path}.capabilities.{name}"),
      )?,
    );
  }
  text(data.get("binary"), &format!("{path}.binary"))?;
  strings(
    data.get("versionCommand"),
    &format!("{path}.versionCommand"),
  )?;
  text(
    data.get("profileIsolation"),
    &format!("{path}.profileIsolation"),
  )?;
  Ok(outcomes)
}

fn header(data: &Map<String, Value>) -> Checked<()> {
  for (key, path) in [
    ("schemaVersion", "evidence.schemaVersion"),
    ("probeContractVersion", "evidence.probeContractVersion"),
  ] {
    if data.get(key).and_then(Value::as_u64) != Some(1) {
      return Err(format!("{path}: expected 1"));
    }
  }
  let source = object(
    data.get("source").unwrap_or(&Value::Null),
    "evidence.source",
  )?;
  exact_keys(source, &["issue", "revision"], "evidence.source")?;
  if text(source.get("issue"), "evidence.source.issue")?
    != "https://github.com/Falconiere/toolu/issues/376"
  {
    return Err("evidence.source.issue: expected issue #376 URL".to_owned());
  }
  let revision = text(source.get("revision"), "evidence.source.revision")?;
  if !Regex::new(r"^[0-9a-f]{40}$").is_ok_and(|re| re.is_match(revision)) {
    return Err("evidence.source.revision: expected full git SHA".to_owned());
  }
  let environment = object(
    data.get("environment").unwrap_or(&Value::Null),
    "evidence.environment",
  )?;
  exact_keys(
    environment,
    &["platform", "arch", "herdrVersion", "topology"],
    "evidence.environment",
  )?;
  for key in ["platform", "arch", "herdrVersion", "topology"] {
    text(environment.get(key), &format!("evidence.environment.{key}"))?;
  }
  timestamp(data.get("generatedAt"), "evidence.generatedAt")
}

/// Every host's capability outcomes, once the whole record is valid.
///
/// # Errors
/// The first violation, with its JSON path.
pub(crate) fn validate(value: &Value) -> Checked<BTreeMap<String, BTreeMap<String, String>>> {
  let data = object(value, "evidence")?;
  exact_keys(
    data,
    &[
      "schemaVersion",
      "probeContractVersion",
      "generatedAt",
      "source",
      "environment",
      "hosts",
    ],
    "evidence",
  )?;
  header(data)?;
  let hosts = object(data.get("hosts").unwrap_or(&Value::Null), "evidence.hosts")?;
  exact_keys(hosts, &HOSTS, "evidence.hosts")?;
  HOSTS
    .iter()
    .map(|name| {
      Ok((
        (*name).to_owned(),
        host(hosts.get(*name), &format!("evidence.hosts.{name}"))?,
      ))
    })
    .collect()
}

/// The three summary lines: validity, outcome counts, and every gap.
pub(crate) fn summarize(hosts: &BTreeMap<String, BTreeMap<String, String>>) -> String {
  let mut counts: BTreeMap<&str, usize> = OUTCOMES.iter().map(|outcome| (*outcome, 0)).collect();
  let mut gaps = Vec::new();
  for name in HOSTS {
    for capability in CAPABILITIES {
      let outcome = hosts
        .get(name)
        .and_then(|caps| caps.get(capability))
        .map_or("unverified", String::as_str);
      if let Some(count) = counts.get_mut(outcome) {
        *count += 1;
      }
      if outcome != "verified" {
        gaps.push(format!("{name}.{capability}={outcome}"));
      }
    }
  }
  let tally: Vec<String> = OUTCOMES
    .iter()
    .map(|name| format!("{name}={}", counts.get(name).copied().unwrap_or(0)))
    .collect();
  let gaps = if gaps.is_empty() {
    "none".to_owned()
  } else {
    gaps.join(", ")
  };
  format!(
    "probe-evidence: valid schema=1 contract=1\noutcomes: {}\ngaps: {gaps}",
    tally.join(" ")
  )
}

/// Check `text` (the committed evidence when `None`) and summarize it.
///
/// # Errors
/// `probe-evidence: invalid: <path>: <reason>`.
pub(crate) fn check_evidence(text: Option<&str>) -> Checked<String> {
  let value: Value = serde_json::from_str(text.unwrap_or(COMMITTED))
    .map_err(|err| format!("probe-evidence: invalid: {err}"))?;
  validate(&value)
    .map(|hosts| summarize(&hosts))
    .map_err(|err| format!("probe-evidence: invalid: {err}"))
}

#[cfg(test)]
#[path = "tests/probe_evidence_test.rs"]
mod tests;
