//! `cargo xtask <task>`: workspace tasks for the toolu Rust rebuild (epic #402).
//!
//! Exit codes: 0 clean, 1 findings, 2 usage, setup or missing-tool error.

mod bench;
mod bench_hooks;
mod bench_hooks_entries;
mod bench_hooks_measure;
mod bench_hooks_report;
mod bench_hooks_sandbox;
mod bench_result;
mod bench_statusline;
mod bench_statusline_repo;
mod bench_statusline_report;
mod check_hooks;
mod check_workflows;
mod ci_aggregate;
mod ci_changes;
mod ci_check;
mod ci_diff;
mod ci_glob;
mod ci_model;
mod ci_yaml;
mod cli_compat;
mod command_tree;
mod context_budget;
mod coverage;
mod data;
mod deploy_gate;
mod deploy_guard;
mod deploy_line;
mod deploy_tag;
mod docs_cli;
mod env_scrub;
mod final_removal;
mod gate;
mod gate_change;
mod gate_coverage;
mod gate_coverage_discover;
mod gate_coverage_names;
mod guardrails;
mod homebrew_formula;
mod hooks_cursor;
mod hooks_entries;
mod hooks_manifests;
mod hooks_muse;
mod host_smoke;
mod host_smoke_checks;
mod host_smoke_claude;
mod host_smoke_codex;
mod host_smoke_cursor;
mod host_smoke_env;
mod host_smoke_exec;
mod host_smoke_muse;
mod host_smoke_observe;
mod host_smoke_project;
mod host_smoke_prompts;
mod host_smoke_steps;
mod launcher_e2e;
mod layers;
mod layers_check;
mod lexer;
mod markdown_cli;
mod measure;
mod metadata;
mod options;
mod output;
mod pack;
mod pack_npm;
mod packaging;
mod packaging_assets;
mod packaging_catalog;
mod packaging_muse;
mod packaging_muse_identity;
mod print_hook;
mod print_muse_hook;
mod process_group;
mod reach;
mod scratch;
mod source;
mod startup;
mod unused_pub;
mod workflow_ci;
mod workflow_install;
mod workflow_release;
mod workflow_yaml;
mod workspace;

use std::process::ExitCode;

use options::Options;

/// What a task concluded when it ran to the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
  /// Nothing to report.
  Clean,
  /// Findings were printed.
  Findings,
}

/// A task: parsed options in, a verdict or a setup error out.
type Task = fn(&Options) -> Result<Verdict, String>;

/// Every `cargo xtask` task. The behaviour inventory discovers its entries here.
const TASKS: &[(&str, Task)] = &[
  ("gate", gate::run),
  ("guardrails", guardrails::run),
  ("check-layers", layers_check::run),
  ("check-reach", reach::run),
  ("check-unused-pub", unused_pub::run),
  ("check-gate-change", gate_change::run),
  ("check-coverage", coverage::run),
  ("measure", measure::run),
  ("print-hook", print_hook::run),
  ("print-muse-hook", print_muse_hook::run),
  ("check-hooks", check_hooks::run),
  ("check-workflows", check_workflows::run),
  ("launcher-e2e", launcher_e2e::run),
  ("docs-cli", docs_cli::run),
  ("check-cli-compat", cli_compat::run),
  ("check-startup", startup::run),
  ("check-markdown-cli", markdown_cli::run),
  ("homebrew-formula", homebrew_formula::run),
  ("deploy-gate", deploy_gate::run),
  ("deploy-guard", deploy_guard::run),
  ("context-budget", context_budget::run),
  ("ci-changes", ci_changes::run),
  ("check-ci-paths", ci_check::run),
  ("ci-aggregate", ci_aggregate::run),
  ("final-removal", final_removal::run),
  ("gate-coverage", gate_coverage::run),
  ("packaging", packaging::run),
  ("bench", bench::run),
  ("bench-hooks", bench_hooks::run),
  ("bench-statusline", bench_statusline::run),
  ("host-smoke", host_smoke::run),
  ("pack", pack::run),
];

// `\x20` keeps the second line's indent: a `\` continuation strips leading spaces.
const USAGE: &str = "usage: cargo xtask <task> [--root DIR] [--base REF] [--title TEXT] \
  [--only STEP]... [--timeout N] [--timeout-ms N] [--resolver] [--bin PATH] [--home DIR] \
  [--check] [ARG]...\n\
  \x20      cargo xtask measure --out FILE -- COMMAND [ARG]...\n\
  tasks: gate, guardrails, check-layers, check-reach, check-unused-pub, check-gate-change, \
  check-coverage, measure, print-hook, print-muse-hook, check-hooks, check-workflows, launcher-e2e, docs-cli, check-cli-compat, \
  check-startup, check-markdown-cli, homebrew-formula, deploy-gate, deploy-guard, context-budget, ci-changes, \
  check-ci-paths, ci-aggregate, final-removal, gate-coverage, packaging, bench, bench-hooks, bench-statusline, host-smoke, pack";

/// Run the task named by `args[0]` and map its outcome to an exit code.
fn run(args: &[String]) -> ExitCode {
  let result = match args.split_first() {
    Some((name, rest)) => match TASKS.iter().find(|(task, _)| task == name) {
      Some((_, task)) => Options::parse(rest)
        .and_then(|options| options.for_task(name))
        .map_err(|err| format!("{err}\n{USAGE}"))
        .and_then(|options| task(&options)),
      None => Err(format!("unknown task {name}\n{USAGE}")),
    },
    None => Err(USAGE.to_owned()),
  };
  match result {
    Ok(Verdict::Clean) => ExitCode::SUCCESS,
    Ok(Verdict::Findings) => ExitCode::from(1),
    Err(message) => {
      output::error(&format!("xtask: {message}"));
      ExitCode::from(2)
    }
  }
}

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  run(&args)
}

#[cfg(test)]
#[path = "tests/task_options_test.rs"]
mod task_options;

#[cfg(test)]
#[path = "tests/main_test.rs"]
mod tests;
