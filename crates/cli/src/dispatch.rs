//! A command line through the clap tree (#442): parse, build the verb context
//! from the global flags, run the namespace, and hold the output contract —
//! under `--json` stdout is one document, and `--quiet` drops the diagnostic of
//! a successful run.

use std::path::PathBuf;

use clap::{ArgMatches, Command};
use toolu_protocol::HOOK_PROTOCOL;
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::{Ctx, Outcome};

use crate::registry::{self, Action};
use crate::{Context, clap_error, commands, fast, hook};

/// Run `words` (argv after the program name) through `tree`.
pub(crate) fn run(words: &[String], context: &Context<'_>, tree: &dyn Fn() -> Command) -> Outcome {
  let argv = std::iter::once("toolu".to_owned()).chain(words.iter().cloned());
  let matches = match tree().try_get_matches_from(argv) {
    Ok(matches) => matches,
    Err(err) => return clap_error::outcome(&err, wants_json(words)),
  };
  let mut ctx = ctx_of(&matches);
  if reads_stdin(&matches) {
    ctx.stdin = (context.stdin)().ok();
  }
  let outcome = route(&matches, &ctx, context, tree);
  // Hooks speak their host's protocol, so the output contract leaves them alone.
  if is_hook(&matches) {
    outcome
  } else {
    finish(outcome, &ctx)
  }
}

fn is_hook(matches: &ArgMatches) -> bool {
  matches
    .subcommand()
    .is_some_and(|(name, sub)| name == "hook" || sub.subcommand_name() == Some("hook"))
}

/// Whether `--json` is on the command line, before any `--`.
pub(crate) fn wants_json(words: &[String]) -> bool {
  words
    .iter()
    .take_while(|word| word.as_str() != "--")
    .any(|word| word == "--json")
}

fn ctx_of(matches: &ArgMatches) -> Ctx {
  Ctx {
    json: matches.get_flag("json"),
    quiet: matches.get_flag("quiet"),
    host: matches
      .get_one::<String>("host")
      .and_then(|name| Host::parse(name)),
    config_dir: matches.get_one::<PathBuf>("config-dir").cloned(),
    stdin: None,
  }
}

/// `toolu jev` reads stdin only for `--state -` or `ask -`.
fn reads_stdin(matches: &ArgMatches) -> bool {
  matches
    .subcommand()
    .is_some_and(|(name, sub)| name == "jev" && toolu_jev::needs_stdin(sub))
}

fn route(
  matches: &ArgMatches,
  ctx: &Ctx,
  context: &Context<'_>,
  tree: &dyn Fn() -> Command,
) -> Outcome {
  let Some((name, sub)) = matches.subcommand() else {
    if matches.get_flag("hook-protocol") {
      return Outcome::data(HOOK_PROTOCOL.to_string());
    }
    let help = tree().render_help().to_string();
    return Outcome::failed(
      Exit::Usage,
      format!("toolu: a command is required\n\n{}", help.trim_end()),
    );
  };
  let Some(namespace) = registry::find(name) else {
    return Outcome::failed(Exit::Failure, format!("toolu: no namespace {name}"));
  };
  if let Some(("hook", hook_matches)) = sub.subcommand() {
    return hook::run(&fast::request(namespace.owner, hook_matches), context);
  }
  match namespace.action {
    Action::Verb(run) => run(sub, ctx),
    Action::Hook => hook::run(&fast::request(namespace.owner, sub), context),
    Action::Commands => commands::run(sub, ctx, tree),
  }
}

/// The output contract on a verb's outcome.
pub(crate) fn finish(mut outcome: Outcome, ctx: &Ctx) -> Outcome {
  let success = outcome.exit == Exit::Success;
  if ctx.json && outcome.stdout.is_none() {
    outcome.stdout = Some(if success {
      "{}".to_owned()
    } else {
      // A failure that printed nothing still says what its exit code means.
      let message = outcome
        .stderr
        .as_deref()
        .map(clap_error::first_line)
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| outcome.exit.meaning());
      clap_error::envelope(outcome.exit, message, None)
    });
  }
  if ctx.quiet && success {
    outcome.stderr = None;
  }
  outcome
}

#[cfg(test)]
#[path = "tests/dispatch_test.rs"]
mod tests;
