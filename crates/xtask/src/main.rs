//! `cargo xtask <task>`: workspace tasks for the toolu Rust rebuild (epic #402).
//!
//! Exit codes: 0 clean, 1 violations found, 2 usage or setup error.

mod check;
mod layers;
mod metadata;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str =
  "usage: cargo xtask check-layers [--manifest-path <Cargo.toml>] [--layers <layers.json>]";

/// The layer table shipped beside this crate.
const DEFAULT_LAYERS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/layers.json");

struct Options {
  manifest: Option<PathBuf>,
  layers: PathBuf,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
  let mut options = Options {
    manifest: None,
    layers: PathBuf::from(DEFAULT_LAYERS),
  };
  let mut rest = args.iter();
  while let Some(flag) = rest.next() {
    let value = rest
      .next()
      .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
    match flag.as_str() {
      "--manifest-path" => options.manifest = Some(PathBuf::from(value)),
      "--layers" => options.layers = PathBuf::from(value),
      _ => return Err(format!("unknown option {flag}\n{USAGE}")),
    }
  }
  Ok(options)
}

fn check_layers(args: &[String]) -> Result<ExitCode, String> {
  let options = parse_options(args)?;
  let text = std::fs::read_to_string(&options.layers)
    .map_err(|err| format!("cannot read {}: {err}", options.layers.display()))?;
  let table = layers::LayerTable::parse(&text)
    .map_err(|err| format!("{}: {err}", options.layers.display()))?;
  let metadata = metadata::load(options.manifest.as_deref())?;
  let report = check::check_layers(&metadata, &table);
  if report.violations.is_empty() {
    println!(
      "check-layers: {} crates, {} edges, ok",
      report.crates, report.edges
    );
    return Ok(ExitCode::SUCCESS);
  }
  for violation in &report.violations {
    eprintln!("check-layers: {violation}");
  }
  Ok(ExitCode::from(1))
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let result = match args.split_first() {
    Some((task, rest)) if task == "check-layers" => check_layers(rest),
    Some((task, _)) => Err(format!("unknown task {task}\n{USAGE}")),
    None => Err(USAGE.to_string()),
  };
  result.unwrap_or_else(|message| {
    eprintln!("xtask: {message}");
    ExitCode::from(2)
  })
}
