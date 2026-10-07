//! `toolu ledger` (#421): the delivery-flow plan ledger and the four-gate
//! verdict, with the verbs, arguments, output and exit codes of the TypeScript
//! CLIs `plugins/toolu/hooks/dist/plan-ledger.js` and `verdict.js`. The logic is
//! `toolu_engine::{ledger, verdict}`; this module parses the command line and
//! maps each result to an `Outcome`.

/// The engine results as `Outcome`s and `--json` documents.
pub mod outcome;

use std::time::SystemTime;

use clap::{Arg, ArgAction, ArgMatches, Command};
use toolu_engine::ledger::commands::{
  ledger_path_command, ledger_root_command, ledger_self_test, ledger_status, require_git,
};
use toolu_engine::ledger::context::RunFlags;
use toolu_engine::ledger::io::LedgerOptions;
use toolu_engine::ledger::preflight::ledger_preflight;
use toolu_engine::ledger::run::ledger_run;
use toolu_engine::verdict::verdict_main;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_dir;

use outcome::{located, ran, reported, self_tested, statused, verdict};

const AFTER_HELP: &str = "Exit codes of run and status: 0 every step is fresh-green, 1 a step is not, 2 an error \
(nothing is written). preflight exits 0 when the plan and its spec are Approved, 1 when not, 2 when the plan \
cannot be read. verdict exits 0 when ready, 1 when blocked, 2 outside a repository.";

fn run_command() -> Command {
  Command::new("run")
    .about("Run the plan's step checks and stamp the branch ledger")
    .args_override_self(true)
    .arg(
      Arg::new("doc")
        .value_name("DOC")
        .required(true)
        .help("The plan doc"),
    )
    .arg(
      Arg::new("step")
        .long("step")
        .value_name("ID")
        .help("Run only this step"),
    )
    .arg(
      Arg::new("activity")
        .long("activity")
        .value_name("LABEL")
        .help("What the running step is doing; needs --step"),
    )
    .arg(
      Arg::new("force")
        .long("force")
        .action(ArgAction::SetTrue)
        .help("Re-run steps that are already fresh-green"),
    )
    .arg(
      Arg::new("verify")
        .long("verify")
        .action(ArgAction::SetTrue)
        .help("Judge every step on the whole branch diff, and stamp the verified hash"),
    )
}

/// The `toolu ledger` namespace.
pub fn command() -> Command {
  Command::new("ledger")
    .about("The plan ledger, verdicts and push waivers")
    .after_help(AFTER_HELP)
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(run_command())
    .subcommand(
      Command::new("status")
        .about("Heal, recompute and print the branch ledger without running checks"),
    )
    .subcommand(
      Command::new("preflight")
        .about("Refuse unless the plan and its declared spec are Approved")
        .arg(
          Arg::new("doc")
            .value_name("DOC")
            .help("The plan doc; the ledger's plan_doc by default"),
        ),
    )
    .subcommand(Command::new("path").about("Print the branch ledger's path"))
    .subcommand(Command::new("root").about("Print the project root"))
    .subcommand(
      Command::new("self-test")
        .long_flag("self-test")
        .about("Parse a built-in two-step plan and check the result"),
    )
    .subcommand(
      Command::new("verdict")
        .about("Judge the four push gates: quality, plan, review and docs")
        .arg(
          Arg::new("mode")
            .value_name("MODE")
            .required(true)
            .value_parser(["status", "json"])
            .help("status prints a table, json the report"),
        ),
    )
}

/// Where a verb runs: the process environment (`--config-dir` as
/// `TOOLU_CONFIG_DIR`), the `--host` or detected host, and the working directory.
fn options(ctx: &Ctx) -> Result<LedgerOptions, Outcome> {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.display().to_string());
  }
  let cwd = current_dir().map_err(|err| {
    Outcome::failed(
      Exit::Blocked,
      format!("toolu ledger: no working directory: {err}"),
    )
  })?;
  Ok(LedgerOptions {
    roots: Roots::new(env, ctx.host),
    cwd,
    now: SystemTime::now,
  })
}

fn text(matches: &ArgMatches, id: &str) -> Option<String> {
  matches.get_one::<String>(id).cloned()
}

fn plan_ledger(name: &str, matches: &ArgMatches, opts: &LedgerOptions, json: bool) -> Outcome {
  if let Some(failed) = require_git(opts) {
    return reported(&failed, None);
  }
  match name {
    "run" => {
      let flags = RunFlags {
        only_step: text(matches, "step"),
        activity: text(matches, "activity"),
        force: matches.get_flag("force"),
        verify: matches.get_flag("verify"),
      };
      ran(
        &ledger_run(&text(matches, "doc").unwrap_or_default(), &flags, opts),
        json,
      )
    }
    "status" => statused(ledger_status(opts), json),
    "preflight" => reported(
      &ledger_preflight(text(matches, "doc").as_deref(), opts),
      None,
    ),
    "path" => located(ledger_path_command(opts), "path", json),
    "root" => located(ledger_root_command(opts), "root", json),
    _ => self_tested(&ledger_self_test(), json),
  }
}

/// Run a `toolu ledger` verb.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let Some((name, sub)) = matches.subcommand() else {
    return Outcome::failed(Exit::Usage, "toolu ledger: a verb is required".to_owned());
  };
  let opts = match options(ctx) {
    Ok(opts) => opts,
    Err(failed) => return failed,
  };
  if name == "verdict" {
    let mode_json = sub
      .get_one::<String>("mode")
      .is_some_and(|mode| mode == "json");
    return verdict(&verdict_main(mode_json, &opts), ctx.json);
  }
  plan_ledger(name, sub, &opts, ctx.json)
}

#[cfg(test)]
#[path = "tests/ledger_test.rs"]
mod tests;
