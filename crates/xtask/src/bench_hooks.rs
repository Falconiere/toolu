//! `cargo xtask bench-hooks [--bin <toolu>] [--runs N] [--only <plugin/hook>]...
//! [--out FILE] [--assert]` (#410, ported from `hook-resources.ts` by #440): every
//! `hooks.json` entry spawned through its real launcher with the fixed payload of
//! `benchmarks/cases/hooks/payloads.json`, two warm-up runs then `--runs`
//! measured ones, each in a fresh `xtask measure`; p50/p90 of max RSS, CPU and
//! wall. `--assert` fails when an entry of `benchmarks/hook-budgets.json` is over
//! its p50 budget. Exit 0 ok, 1 over budget, 2 usage or setup error.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Map, Value, json};

use crate::bench_hooks_entries::{HookEntry, discover, select};
use crate::bench_hooks_measure::{MEASURER_VAR, SPAWN_DEADLINE, Spawn, measurer, samples};
use crate::bench_hooks_report::{Row, stale_budgets, summarize, table, violations};
use crate::bench_hooks_sandbox::Sandbox;
use crate::options::Options;
use crate::{Verdict, output};

/// Unmeasured spawns before the measured ones.
const WARMUP: usize = 2;

/// The issue's CI limit for one run's measured phase, in milliseconds.
const MAX_ELAPSED_MS: u128 = 60_000;

/// Measure, print, write and judge.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = &options.root;
  let bin = toolu_bin(options)?;
  let runs = runs(options)?;
  let payloads = entries_of(&read_json(
    &root.join("benchmarks/cases/hooks/payloads.json"),
  )?)?;
  let discovered = discover(root)?;
  let known: Vec<String> = discovered.iter().map(|entry| entry.id.clone()).collect();
  let entries = select(discovered, &payloads, &options.only)?;
  let measurer =
    measurer(root, std::env::var_os(MEASURER_VAR)).map_err(|err| format!("bench-hooks: {err}"))?;
  let started = Instant::now();
  let rows = measure_all(
    &Bench {
      root,
      bin: &bin,
      measurer: &measurer,
      runs,
    },
    &entries,
    &payloads,
  )?;
  let elapsed = started.elapsed().as_millis();
  output::say(&table(&rows));
  if let Some(out) = &options.out {
    write_result(out, &provenance(root, &bin, runs, elapsed), &rows)?;
  }
  if !options.assert {
    return Ok(Verdict::Clean);
  }
  let budgets = entries_of(&read_json(&root.join("benchmarks/hook-budgets.json"))?)?;
  let mut found = stale_budgets(&known, &budgets);
  found.extend(violations(&rows, &budgets));
  if elapsed > MAX_ELAPSED_MS {
    found.push(format!(
      "the measured phase took {elapsed} ms > {MAX_ELAPSED_MS} ms"
    ));
  }
  Ok(output::findings("bench-hooks", &found))
}

struct Bench<'b> {
  root: &'b Path,
  bin: &'b Path,
  measurer: &'b Path,
  runs: usize,
}

fn toolu_bin(options: &Options) -> Result<PathBuf, String> {
  let bin = options
    .bin
    .clone()
    .unwrap_or_else(|| options.root.join("target/release/toolu"));
  if bin.is_file() {
    return std::fs::canonicalize(&bin).map_err(|err| format!("bench-hooks: {err}"));
  }
  Err(format!(
    "bench-hooks: no toolu at {}; build it with cargo build --release --locked --bin toolu",
    bin.display()
  ))
}

fn runs(options: &Options) -> Result<usize, String> {
  match &options.runs {
    None => Ok(10),
    Some(text) => text
      .parse::<usize>()
      .ok()
      .filter(|runs| *runs > 0)
      .ok_or_else(|| format!("--runs takes a whole number above 0, got {text}")),
  }
}

/// Every entry in the full sandbox, except `toolu/post-tools`, whose budget
/// excludes the quality linters other plugins register, in a core-only one.
fn measure_all(
  bench: &Bench<'_>,
  entries: &[HookEntry],
  payloads: &Map<String, Value>,
) -> Result<Vec<Row>, String> {
  let full = Sandbox::new(bench.root, bench.bin, true)?;
  let core = Sandbox::new(bench.root, bench.bin, false)?;
  let mut rows = Vec::new();
  for entry in entries {
    let sandbox = if entry.id == "toolu/post-tools" {
      &core
    } else {
      &full
    };
    let stdin = payload_stdin(payloads.get(&entry.id), &sandbox.project)?;
    let spawn = Spawn {
      argv: vec!["/bin/sh".to_owned(), "-c".to_owned(), entry.command.clone()],
      cwd: &sandbox.project,
      env: sandbox.env(&entry.root),
      stdin,
      report_dir: sandbox.scratch(),
      deadline: SPAWN_DEADLINE,
    };
    let measured = samples(bench.measurer, &spawn, &entry.id, (WARMUP, bench.runs))?;
    rows.push(summarize(&entry.id, &entry.event, &measured));
  }
  Ok(rows)
}

/// The payload's `stdin` as JSON text, every `${PROJECT}` spliced as a JSON-escaped path.
fn payload_stdin(payload: Option<&Value>, project: &Path) -> Result<String, String> {
  let stdin = payload
    .and_then(|value| value.get("stdin"))
    .ok_or("bench-hooks: a payload lacks stdin")?;
  let path =
    serde_json::to_string(&project.display().to_string()).map_err(|err| err.to_string())?;
  let inner = path
    .get(1..path.len().saturating_sub(1))
    .unwrap_or_default();
  Ok(stdin.to_string().replace("${PROJECT}", inner))
}

fn read_json(path: &Path) -> Result<Value, String> {
  let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
  serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

fn entries_of(doc: &Value) -> Result<Map<String, Value>, String> {
  doc
    .get("entries")
    .and_then(Value::as_object)
    .cloned()
    .ok_or_else(|| "bench-hooks: a data file lacks entries".to_owned())
}

fn provenance(root: &Path, bin: &Path, runs: usize, elapsed: u128) -> Value {
  let first_line = |program: &Path, args: &[&str]| {
    std::process::Command::new(program)
      .args(args)
      .current_dir(root)
      .output()
      .ok()
      .map(|out| {
        String::from_utf8_lossy(&out.stdout)
          .lines()
          .next()
          .unwrap_or("")
          .trim()
          .to_owned()
      })
      .unwrap_or_default()
  };
  let runner = if std::env::var_os("GITHUB_ACTIONS").is_some() {
    "github-actions"
  } else {
    "local"
  };
  json!({
    "date": first_line(Path::new("date"), &["-u", "+%Y-%m-%d"]),
    "commit": first_line(Path::new("git"), &["rev-parse", "HEAD"]),
    "platform": if std::env::consts::OS == "macos" { "darwin" } else { std::env::consts::OS },
    "arch": match std::env::consts::ARCH { "aarch64" => "arm64", "x86_64" => "x64", other => other },
    "toolu": first_line(bin, &["--version"]),
    "runner": runner,
    "runs": runs,
    "warmup": WARMUP,
    "elapsedMs": elapsed,
  })
}

fn write_result(out: &Path, provenance: &Value, rows: &[Row]) -> Result<(), String> {
  let doc = json!({"schema": "toolu.hook-resources/v1", "provenance": provenance, "entries": rows});
  let text = serde_json::to_string_pretty(&doc).map_err(|err| err.to_string())?;
  if let Some(dir) = out.parent().filter(|dir| !dir.as_os_str().is_empty()) {
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  }
  std::fs::write(out, text + "\n").map_err(|err| format!("{}: {err}", out.display()))
}

#[cfg(test)]
#[path = "tests/bench_hooks_test.rs"]
mod tests;
