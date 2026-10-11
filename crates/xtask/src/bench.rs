//! `cargo xtask bench`: the deterministic retrieval tier, plus live and container.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value};

use crate::bench_result::write_result;
use crate::ci_yaml::bun_binary;
use crate::options::Options;
use crate::{Verdict, output};

const PRICING_ID: &str = "2026-06-15";
const EXACT: &str = "bench: exact token counts are not available";

struct Request<'a> {
  root: &'a Path,
  queries: &'a Path,
  corpus: &'a Path,
  out_dir: &'a Path,
  api_key: Option<&'a str>,
}

struct Query {
  id: String,
  target: String,
  pattern: String,
  lang: String,
}

struct Measured {
  case: Value,
  modes: Vec<String>,
  base: i64,
  treat: i64,
  note: String,
}

/// `bench deterministic`, `bench live`, or `bench container`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  match options.files.as_slice() {
    [mode] if mode == Path::new("deterministic") => deterministic_cli(options),
    [mode] if mode == Path::new("live") => {
      spawn_script(options, "tooling/src/benchmarks/cases/whole-session.ts")
    }
    [mode] if mode == Path::new("container") => {
      spawn_script(options, "tooling/src/benchmarks/linux-container.ts")
    }
    _ => Err("usage: cargo xtask bench <deterministic|live|container>".to_owned()),
  }
}

fn deterministic_cli(options: &Options) -> Result<Verdict, String> {
  let queries = options.root.join("benchmarks/cases/retrieval/queries.tsv");
  let key = std::env::var("ANTHROPIC_API_KEY")
    .ok()
    .filter(|value| !value.is_empty());
  let path = deterministic(&Request {
    root: &options.root,
    queries: &queries,
    corpus: &options.root,
    out_dir: &options.root.join("benchmarks/results"),
    api_key: key.as_deref(),
  })?;
  output::say(&path.display().to_string());
  Ok(Verdict::Clean)
}

fn deterministic(request: &Request<'_>) -> Result<PathBuf, String> {
  if request.api_key.is_some_and(|key| !key.is_empty()) {
    return Err(EXACT.to_owned());
  }
  if !request.queries.is_file() {
    return Err(format!(
      "bench: queries file not found: {}",
      request.queries.display()
    ));
  }
  let measured = measure_all(request)?;
  let doc = document(request.root, &measured)?;
  let modes = measured
    .iter()
    .flat_map(|item| item.modes.clone())
    .collect::<Vec<_>>();
  write_result(&doc, &modes, request.out_dir)
}

fn measure_all(request: &Request<'_>) -> Result<Vec<Measured>, String> {
  let text = std::fs::read_to_string(request.queries)
    .map_err(|err| format!("bench: cannot read {}: {err}", request.queries.display()))?;
  let mut measured = Vec::new();
  for line in text.lines() {
    if let Some(query) = query(line) {
      measured.push(measure(request.corpus, &query)?);
    }
  }
  Ok(measured)
}

fn query(line: &str) -> Option<Query> {
  let mut parts = line.split('\t');
  let id = parts.next().unwrap_or("");
  if id.is_empty() || id.starts_with('#') {
    return None;
  }
  Some(Query {
    id: id.to_owned(),
    target: parts.next().unwrap_or("").to_owned(),
    pattern: parts.next().unwrap_or("").to_owned(),
    lang: parts.next().unwrap_or("").to_owned(),
  })
}

fn measure(corpus: &Path, query: &Query) -> Result<Measured, String> {
  let file = corpus.join(&query.target);
  if !file.is_file() || file.metadata().is_ok_and(|meta| meta.len() == 0) {
    return Ok(guarded(query));
  }
  let body = std::fs::read_to_string(&file)
    .map_err(|err| format!("bench: cannot read {}: {err}", file.display()))?;
  let matched = ast_grep(&query.pattern, &query.lang, &file)?;
  let base = tokens(&body);
  let treat = tokens(&matched);
  Ok(Measured {
    case: counted(&query.id, base, treat),
    modes: vec!["heuristic".to_owned()],
    base,
    treat,
    note: String::new(),
  })
}

fn guarded(query: &Query) -> Measured {
  let mut case = counted(&query.id, 0, 0);
  if let Some(obj) = case.as_object_mut() {
    obj.insert(
      "note".to_owned(),
      Value::String("missing-or-empty-target".to_owned()),
    );
  }
  Measured {
    case,
    modes: Vec::new(),
    base: 0,
    treat: 0,
    note: format!("{}:missing-target; ", query.id),
  }
}

fn counted(id: &str, base: i64, treat: i64) -> Value {
  serde_json::json!({
    "id": id,
    "baseline_tokens": base,
    "treatment_tokens": treat,
    "saved": percent(base, treat),
  })
}

fn ast_grep(pattern: &str, lang: &str, file: &Path) -> Result<String, String> {
  let output = Command::new("ast-grep")
    .args(["run", "--lang", lang, "--pattern", pattern])
    .arg(file)
    .output()
    .map_err(|err| format!("bench: ast-grep could not run: {err}"))?;
  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn tokens(text: &str) -> i64 {
  let body = text.trim_end_matches('\n');
  i64::try_from(body.len() / 4).unwrap_or(i64::MAX)
}

fn percent(base: i64, treat: i64) -> i64 {
  if base <= 0 {
    return 0;
  }
  base.saturating_sub(treat).saturating_mul(100) / base
}

fn document(root: &Path, measured: &[Measured]) -> Result<Value, String> {
  let base = measured
    .iter()
    .fold(0_i64, |sum, item| sum.saturating_add(item.base));
  let treat = measured
    .iter()
    .fold(0_i64, |sum, item| sum.saturating_add(item.treat));
  let notes = measured
    .iter()
    .map(|item| item.note.as_str())
    .collect::<String>();
  Ok(serde_json::json!({
    "mechanism": "retrieval",
    "tier": "deterministic",
    "method": "tool-bytes",
    "tokenizer": { "mode": "heuristic", "source": "bytes-div-4" },
    "provenance": {
      "model": Value::Null,
      "date": today()?,
      "commit": head_commit(root),
      "n_runs": 1,
      "pricing_id": PRICING_ID,
    },
    "baseline": { "label": "full-read", "tokens": totals(base), "cost": Value::Null },
    "treatment": { "label": "ast-grep", "tokens": totals(treat), "cost": Value::Null },
    "delta": {
      "tokens_pct": percent(base, treat),
      "cost_pct": Value::Null,
      "abs_tokens": base.saturating_sub(treat),
      "mean": Value::Null,
      "stddev": Value::Null,
    },
    "cases": measured.iter().map(|item| item.case.clone()).collect::<Vec<_>>(),
    "notes": notes,
  }))
}

fn totals(total: i64) -> Map<String, Value> {
  Map::from_iter([
    ("input".to_owned(), Value::from(total)),
    ("output".to_owned(), Value::from(0)),
    ("cache_read".to_owned(), Value::from(0)),
    ("cache_write".to_owned(), Value::from(0)),
    ("total".to_owned(), Value::from(total)),
  ])
}

fn today() -> Result<String, String> {
  let output = Command::new("date")
    .arg("+%Y-%m-%d")
    .output()
    .map_err(|err| format!("bench: date: {err}"))?;
  if !output.status.success() {
    return Err("bench: date failed".to_owned());
  }
  Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn head_commit(root: &Path) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(root)
    .args(["rev-parse", "HEAD"])
    .output();
  match output {
    Ok(output) if output.status.success() => {
      String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
    _ => "unknown".to_owned(),
  }
}

fn spawn_script(options: &Options, rel: &str) -> Result<Verdict, String> {
  let bun = bun_binary()?;
  let status = Command::new(bun)
    .arg(options.root.join(rel))
    .current_dir(&options.root)
    .status()
    .map_err(|err| format!("bench: cannot run {rel}: {err}"))?;
  match status.code() {
    Some(0) => Ok(Verdict::Clean),
    Some(1) => Ok(Verdict::Findings),
    Some(code) => Err(format!("bench: {rel} exited {code}")),
    None => Err(format!("bench: {rel} was terminated")),
  }
}

#[cfg(test)]
#[path = "tests/bench_test.rs"]
mod tests;
