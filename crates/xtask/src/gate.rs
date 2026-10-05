//! `cargo xtask gate`: every check of the quality bar, in a fixed order,
//! stopping at the first failure. CI requires it; it is what runs locally.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::options::Options;
use crate::{Verdict, coverage, data, gate_change, guardrails, layers_check, output};
use crate::{reach, unused_pub};

/// The steps, in the order they run.
pub(crate) const STEPS: &[&str] = &[
  "gate-change",
  "fmt",
  "clippy",
  "guardrails",
  "layers",
  "reach",
  "deny",
  "machete",
  "unused-pub",
  "jscpd",
  "rust-quality",
  "tests",
  "coverage",
  "docs",
];

/// Run every step, or the `--only` ones, in order.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  if let Some(unknown) = options
    .only
    .iter()
    .find(|step| !STEPS.contains(&step.as_str()))
  {
    return Err(format!(
      "unknown gate step {unknown}; steps: {}",
      STEPS.join(", ")
    ));
  }
  let started = Instant::now();
  for step in STEPS
    .iter()
    .filter(|step| options.only.is_empty() || options.only.iter().any(|only| only == *step))
  {
    let clock = Instant::now();
    output::say(&format!("gate: {step}"));
    if step_run(step, options)? == Verdict::Findings {
      output::error(&format!("gate: {step} failed"));
      return Ok(Verdict::Findings);
    }
    output::say(&format!(
      "gate: {step} ok ({:.1}s)",
      clock.elapsed().as_secs_f64()
    ));
  }
  output::say(&format!(
    "gate: ok ({:.1}s)",
    started.elapsed().as_secs_f64()
  ));
  Ok(Verdict::Clean)
}

fn step_run(step: &str, options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  match step {
    "gate-change" => gate_change::run(options),
    "fmt" => cargo(root, &["fmt", "--all", "--check"]),
    "clippy" => cargo(
      root,
      &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
      ],
    ),
    "guardrails" => guardrails::run(options),
    "layers" => layers_check::run(options),
    "reach" => reach::run(options),
    "deny" => {
      require(root, "cargo-deny", &cargo_bin(), &["deny", "--version"])?;
      cargo(root, &["deny", "--all-features", "check"])
    }
    "machete" => {
      require(
        root,
        "cargo-machete",
        "cargo-machete".as_ref(),
        &["--version"],
      )?;
      status(root, "cargo-machete".as_ref(), &[], &[])
    }
    "unused-pub" => unused_pub::run(options),
    "jscpd" => jscpd(root),
    "rust-quality" => rust_quality(root),
    "tests" => tests(root),
    "coverage" => coverage_step(options),
    _ => docs(root),
  }
}

fn cargo_bin() -> std::ffi::OsString {
  std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// Run `program args` in `root` with inherited output; a non-zero exit is a finding.
fn status(
  root: &Path,
  program: &std::ffi::OsStr,
  args: &[&str],
  env: &[(&str, &str)],
) -> Result<Verdict, String> {
  let status = Command::new(program)
    .args(args)
    .envs(env.iter().copied())
    .current_dir(root)
    .status()
    .map_err(|err| format!("cannot run {}: {err}", program.to_string_lossy()))?;
  Ok(if status.success() {
    Verdict::Clean
  } else {
    Verdict::Findings
  })
}

fn cargo(root: &Path, args: &[&str]) -> Result<Verdict, String> {
  status(root, &cargo_bin(), args, &[])
}

/// Fail closed when a tool is missing: `program probe` must succeed.
fn require(
  root: &Path,
  tool: &str,
  program: &std::ffi::OsStr,
  probe: &[&str],
) -> Result<(), String> {
  let output = Command::new(program).args(probe).current_dir(root).output();
  let detail = match output {
    Ok(output) if output.status.success() => return Ok(()),
    Ok(output) => String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    Err(err) => err.to_string(),
  };
  Err(format!(
    "{tool} is not installed: cargo install {tool} --locked (CI installs it with \
     taiki-e/install-action) — {detail}"
  ))
}

fn jscpd(root: &Path) -> Result<Verdict, String> {
  let local = root.join("node_modules/.bin/jscpd");
  let program = if local.is_file() {
    local.into_os_string()
  } else {
    "jscpd".into()
  };
  let config = format!("{}/jscpd.json", data::DATA_DIR);
  status(root, &program, &["--config", &config, "crates"], &[])
    .map_err(|err| format!("{err} — jscpd is missing: run `bun install` (it is a devDependency)"))
}

fn rust_quality(root: &Path) -> Result<Verdict, String> {
  status(
    root,
    "bun".as_ref(),
    &["tooling/src/check-rust-quality.ts"],
    &[],
  )
  .map_err(|err| format!("{err} — the rust-quality step needs Bun 1.4"))
}

fn tests(root: &Path) -> Result<Verdict, String> {
  require(
    root,
    "cargo-llvm-cov",
    &cargo_bin(),
    &["llvm-cov", "--version"],
  )?;
  for args in [
    &["llvm-cov", "clean", "--workspace"][..],
    &["llvm-cov", "--no-report", "--workspace", "--locked"][..],
    &["test", "--doc", "--workspace", "--locked"][..],
  ] {
    if cargo(root, args)? == Verdict::Findings {
      return Ok(Verdict::Findings);
    }
  }
  Ok(Verdict::Clean)
}

/// Report the coverage of the `tests` step and judge it.
fn coverage_step(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  require(
    root,
    "cargo-llvm-cov",
    &cargo_bin(),
    &["llvm-cov", "--version"],
  )?;
  let report = Command::new(cargo_bin())
    .args(["llvm-cov", "report", "--json", "--summary-only"])
    .current_dir(root)
    .output()
    .map_err(|err| format!("cannot run cargo llvm-cov report: {err}"))?;
  if !report.status.success() {
    output::error(&String::from_utf8_lossy(&report.stderr));
    return Ok(Verdict::Findings);
  }
  let export: coverage::Export = serde_json::from_slice(&report.stdout)
    .map_err(|err| format!("cargo llvm-cov report printed unreadable JSON: {err}"))?;
  coverage::check(options, &export)
}

fn docs(root: &Path) -> Result<Verdict, String> {
  status(
    root,
    &cargo_bin(),
    &["doc", "--workspace", "--no-deps", "--locked"],
    &[("RUSTDOCFLAGS", "-D warnings")],
  )
}

#[cfg(test)]
#[path = "tests/gate_test.rs"]
mod tests;
