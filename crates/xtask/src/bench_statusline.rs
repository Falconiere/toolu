//! `cargo xtask bench-statusline [--bin <toolu>] [--runs N] [--out FILE] [--assert]`
//! (#431, ported from `statusline.ts` by #440): `toolu statusline render` fed the
//! payload of `fixtures/statusline/performance.json` in a linked worktree, three
//! warm-up renders then `--runs` (30) measured ones, each in a fresh `xtask measure`
//! after a cache refresh, then one cold first render. `--out` writes the
//! `toolu.statusline-benchmark/v1` result. `--assert` fails when the warm p50 is over
//! 4000 us wall or 6 MiB max RSS. Exit 0 ok, 1 over budget, 2 usage or setup error.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bench_hooks_measure::{MEASURER_VAR, SPAWN_DEADLINE, Spawn, measurer, samples};
use crate::bench_hooks_report::{Sample, percentile};
use crate::bench_statusline_repo::{Fixture, payload};
use crate::bench_statusline_report::{Measured, WARMUP, cpu_model, document, violations};
use crate::options::Options;
use crate::{Verdict, output};

/// Measured renders unless `--runs` says otherwise.
const RUNS: usize = 30;

/// Measure, print, write and judge.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let bin = toolu_bin(options)?;
  let runs = runs(options)?;
  let fixture_json = options.root.join("fixtures/statusline/performance.json");
  let fixture_json = std::fs::read_to_string(&fixture_json)
    .map_err(|err| format!("bench-statusline: {}: {err}", fixture_json.display()))?;
  let exe = measurer(&options.root, std::env::var_os(MEASURER_VAR))
    .map_err(|err| format!("bench-statusline: {err}"))?;
  let fixture = Fixture::new()?;
  let stdin = payload(&fixture_json, &fixture.worktree)?;
  let measured = measure(&exe, &bin, &fixture, &stdin, runs)?;
  output::say(&format!(
    "statusline p50: {} us, {} bytes",
    measured.wall_us, measured.max_rss_bytes
  ));
  if let Some(out) = &options.out {
    write_result(out, &measured)?;
  }
  if !options.assert {
    return Ok(Verdict::Clean);
  }
  Ok(output::findings("bench-statusline", &violations(&measured)))
}

fn toolu_bin(options: &Options) -> Result<PathBuf, String> {
  let bin = options
    .bin
    .clone()
    .unwrap_or_else(|| options.root.join("target/release/toolu"));
  if bin.is_file() {
    return std::fs::canonicalize(&bin).map_err(|err| format!("bench-statusline: {err}"));
  }
  Err(format!(
    "bench-statusline: no toolu at {}; build it with cargo build --release --locked --bin toolu",
    bin.display()
  ))
}

fn runs(options: &Options) -> Result<usize, String> {
  match &options.runs {
    None => Ok(RUNS),
    Some(text) => text
      .parse::<usize>()
      .ok()
      .filter(|runs| *runs > 0)
      .ok_or_else(|| format!("--runs takes a whole number above 0, got {text}")),
  }
}

/// The warm renders (the cache refreshed before each) and the cold first render.
pub(crate) fn measure(
  measurer: &Path,
  bin: &Path,
  fixture: &Fixture,
  stdin: &str,
  runs: usize,
) -> Result<Measured, String> {
  let spawn = Spawn {
    argv: vec![
      bin.display().to_string(),
      "statusline".to_owned(),
      "render".to_owned(),
    ],
    cwd: &fixture.worktree,
    env: fixture.env(bin, &fixture.cfg),
    stdin: stdin.to_owned(),
    report_dir: fixture.scratch(),
    deadline: SPAWN_DEADLINE,
  };
  let mut warm = Vec::new();
  for round in 0..WARMUP + runs {
    prime(bin, &spawn)?;
    let label = format!("statusline render {}", round + 1);
    let kept = samples(measurer, &spawn, &label, (0, 1))?;
    if round >= WARMUP {
      warm.extend(kept);
    }
  }
  let cold_spawn = Spawn {
    env: fixture.env(bin, &fixture.cold_cfg),
    argv: spawn.argv.clone(),
    stdin: spawn.stdin.clone(),
    ..spawn
  };
  let cold = samples(
    measurer,
    &cold_spawn,
    "statusline cold first render",
    (0, 1),
  )?;
  let cold = cold
    .first()
    .copied()
    .ok_or("bench-statusline: no cold sample")?;
  Ok(summary(&warm, runs, cold))
}

fn summary(warm: &[Sample], runs: usize, cold: Sample) -> Measured {
  let pick = |of: fn(&Sample) -> u64| warm.iter().map(of).collect::<Vec<_>>();
  Measured {
    runs,
    wall_us: percentile(&pick(|s| s.wall_us), 50),
    max_rss_bytes: percentile(&pick(|s| s.max_rss_bytes), 50),
    cold,
  }
}

/// Refresh the statusline cache for the worktree, as the render reads it.
fn prime(bin: &Path, spawn: &Spawn<'_>) -> Result<(), String> {
  let output = Command::new(bin)
    .args(["statusline", "refresh-cache"])
    .arg(spawn.cwd)
    .current_dir(spawn.cwd)
    .env_clear()
    .envs(spawn.env.iter().map(|(key, value)| (key, value)))
    .output()
    .map_err(|err| format!("bench-statusline: cannot prime the cache: {err}"))?;
  if output.status.success() {
    return Ok(());
  }
  let stderr = String::from_utf8_lossy(&output.stderr);
  Err(format!(
    "bench-statusline: cannot prime statusline cache: {}",
    stderr.trim()
  ))
}

fn write_result(out: &Path, measured: &Measured) -> Result<(), String> {
  let text = serde_json::to_string_pretty(&document(measured, cpu_model()))
    .map_err(|err| format!("bench-statusline: {err}"))?;
  if let Some(dir) = out.parent().filter(|dir| !dir.as_os_str().is_empty()) {
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  }
  std::fs::write(out, text + "\n").map_err(|err| format!("{}: {err}", out.display()))
}

#[cfg(test)]
#[path = "tests/bench_statusline_test.rs"]
mod tests;
