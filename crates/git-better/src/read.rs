//! The read verbs: `log` (a range, commit by commit), `context` (the branch
//! against its base) and `pr` (a draft title, body and squash message).

use clap::ArgMatches;
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::host::roots::Roots;

use crate::cli::Place;
use crate::context::{Context, gather};
use crate::pr::draft;
use crate::settings;
use crate::views::{commit_json, commit_text, context_json, context_text};

/// The commits `log` and `context` read when `--limit` is not given.
pub(crate) const DEFAULT_LIMIT: usize = 20;
/// The commits `pr` reads when `--limit` is not given: the most `--limit` takes.
const PR_LIMIT: usize = 200;

fn context_of(
  args: &ArgMatches,
  ctx: &Ctx,
  place: &Place,
  range: Option<&str>,
  default: usize,
) -> Result<Context, Outcome> {
  let (env, cwd) = (&place.env, &place.dir);
  let roots = Roots::new(env.clone(), ctx.host);
  let settings = settings::load(&roots, Some(cwd), None);
  let limit = args
    .get_one::<u64>("limit")
    .and_then(|limit| usize::try_from(*limit).ok())
    .unwrap_or(default);
  gather(env, cwd, &settings, range, limit).map_err(|why| Outcome::failed(Exit::Failure, why))
}

/// Run `log`.
pub(crate) fn log(args: &ArgMatches, ctx: &Ctx, place: &Place) -> Outcome {
  let range = args.get_one::<String>("range").map(String::as_str);
  let context = match context_of(args, ctx, place, range, DEFAULT_LIMIT) {
    Ok(context) => context,
    Err(failed) => return failed,
  };
  if ctx.json {
    let commits: Vec<_> = context.commits.iter().map(commit_json).collect();
    let document = json!({
      "namespace": crate::PLUGIN,
      "verb": "log",
      "range": context.range,
      "commits": commits,
    });
    return Outcome::data(document.to_string());
  }
  if context.commits.is_empty() {
    return Outcome::data(format!("no commits in {}", context.range));
  }
  let blocks: Vec<String> = context.commits.iter().map(commit_text).collect();
  Outcome::data(blocks.join("\n"))
}

/// Run `context`.
pub(crate) fn context(args: &ArgMatches, ctx: &Ctx, place: &Place) -> Outcome {
  match context_of(args, ctx, place, None, DEFAULT_LIMIT) {
    Ok(context) if ctx.json => Outcome::data(context_json(&context).to_string()),
    Ok(context) => Outcome::data(context_text(&context)),
    Err(failed) => failed,
  }
}

/// Run `pr`.
pub(crate) fn pr(args: &ArgMatches, ctx: &Ctx, place: &Place) -> Outcome {
  let context = match context_of(args, ctx, place, None, PR_LIMIT) {
    Ok(context) => context,
    Err(failed) => return failed,
  };
  if context.on_base {
    let why = if context.branch == context.base {
      format!(
        "{} is the base branch: there are no branch commits to draft a PR from",
        context.branch
      )
    } else {
      format!(
        "the base branch {} exists neither locally nor on origin, so the branch's own commits are unknown",
        context.base
      )
    };
    return Outcome::failed(Exit::Failure, why);
  }
  let found = draft(&context);
  if ctx.json {
    let document = json!({
      "namespace": crate::PLUGIN,
      "verb": "pr",
      "branch": context.branch,
      "base": context.base,
      "title": found.title,
      "body": found.body,
      "squash": found.squash,
    });
    return Outcome::data(document.to_string());
  }
  if args.get_flag("squash") {
    return Outcome::data(found.squash);
  }
  Outcome::data(format!("{}\n\n{}", found.title, found.body))
}

#[cfg(test)]
#[path = "tests/read_test.rs"]
mod tests;
