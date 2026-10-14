//! `toolu plugins install|list|remove|update` (#438): the plugin installer, ported
//! from the former Node CLI that npm published as `@toolu/plugins`.
//! Claude Code and Codex are driven through their own plugin CLIs. The npm
//! package is now a wrapper that runs these verbs. This crate never touches the standard streams: a verb
//! returns one `Outcome`, and prompts use the terminal (`/dev/tty`).

use clap::{ArgMatches, Command};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;

mod args;
mod catalog;
mod dispatch;
mod fail;
mod host;
mod prompt;
mod replay;
mod report;
mod step;
#[cfg(test)]
#[path = "tests/support_test.rs"]
mod support;
mod verbs;

use dispatch::{Context, Report};
use fail::Fail;

/// The `toolu plugins` namespace.
pub fn command() -> Command {
  args::command()
}

/// Run a `toolu plugins` verb in the process's environment.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let Some(request) = args::request(matches) else {
    return Outcome::failed(Exit::Usage, "toolu: a plugins verb is required".to_owned());
  };
  let mut terminal = prompt::terminal(!request.no_input && !ctx.json);
  let mut cx = Context {
    env: Env::process(),
    host: ctx.host,
    ask: terminal
      .as_mut()
      .map(|prompter| prompter as &mut dyn prompt::Ask),
    timeout: host::run::HOST_TIMEOUT,
    version: env!("CARGO_PKG_VERSION"),
  };
  let report = catalog::load(request.catalog.as_deref())
    .and_then(|catalog| dispatch::dispatch(&request, &catalog, &mut cx));
  outcome(report, ctx)
}

/// A report as the binary prints it: the text, or under `--json` one document;
/// exit 1 when a step failed. The binary ends stdout with a newline, so the
/// report's own last one is dropped, and an empty report prints nothing.
fn outcome(report: Result<Report, Fail>, ctx: &Ctx) -> Outcome {
  match report {
    Ok(report) => {
      let text = report
        .text
        .strip_suffix('\n')
        .unwrap_or(&report.text)
        .to_owned();
      let stdout = if ctx.json {
        Some(report.json.to_string())
      } else {
        Some(text).filter(|text| !text.is_empty())
      };
      Outcome {
        exit: if report.failed {
          Exit::Failure
        } else {
          Exit::Success
        },
        stdout,
        stderr: None,
      }
    }
    Err(fail) => Outcome::failed(fail.exit, format!("toolu: {}", fail.message)),
  }
}

#[cfg(test)]
#[path = "tests/lib_test.rs"]
mod tests;
