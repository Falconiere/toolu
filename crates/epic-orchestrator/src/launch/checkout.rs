//! The issue's local checkout (`prepareCheckout`), the GitHub budget guard
//! (`guardBudget`), and the shell quoting of dry-run lines (`shellJoin`).

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use regex::Regex;
use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_state::lock::{LockOptions, with_lock};

use super::failure::{Failure, argv, run_checked};
use crate::graph::{Graph, GraphIssue};
use crate::tracker::Gh;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

/// `argv` as one shell line, quoting what the shell would split or expand.
pub(crate) fn shell_join(argv: &[String]) -> String {
  let plain = Regex::new(r"^[A-Za-z0-9_./:=,@%+-]+$");
  argv
    .iter()
    .map(|arg| {
      if arg.is_empty() {
        "''".to_owned()
      } else if plain.as_ref().is_ok_and(|re| re.is_match(arg)) {
        arg.clone()
      } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
      }
    })
    .collect::<Vec<_>>()
    .join(" ")
}

/// Clone a missing checkout into the clone root, find the default branch and
/// fetch it. A dry run logs the commands, assumes `main` and calls nothing.
///
/// # Errors
/// A malformed repository, or a failed clone, lookup or fetch.
pub(crate) fn prepare_checkout(
  graph: &Graph,
  issue: &GraphIssue,
  dry: bool,
  log: &mut Vec<String>,
  github: (&Env, Result<&Gh, String>),
) -> Result<(String, String), Failure> {
  let (env, gh) = github;
  let checkout = match &issue.checkout {
    Some(checkout) => checkout.clone(),
    None => clone_checkout(graph, issue, dry, log, env)?,
  };
  let base = if dry {
    log.push("# dry-run: skip the default-branch lookup; assume main".to_owned());
    "main".to_owned()
  } else {
    default_branch(gh, &issue.repo)?
  };
  let fetch = argv(&["git", "-C", &checkout, "fetch", "origin", &base]);
  log.push(shell_join(&fetch));
  if !dry {
    run_checked(&fetch, None, env, COMMAND_TIMEOUT)?;
  }
  Ok((checkout, base))
}

/// Clone a missing checkout into the clone root (a dry run only logs it).
fn clone_checkout(
  graph: &Graph,
  issue: &GraphIssue,
  dry: bool,
  log: &mut Vec<String>,
  env: &Env,
) -> Result<String, Failure> {
  let parts: Vec<&str> = issue.repo.split('/').collect();
  let Some(name) = parts.as_slice().get(1).filter(|name| {
    parts.len() == 2 && !name.is_empty() && parts.first().is_some_and(|owner| !owner.is_empty())
  }) else {
    return Err(Failure::unknown(format!("bad repo: {}", issue.repo)));
  };
  let checkout = Path::new(&graph.clone_root)
    .join(name)
    .display()
    .to_string();
  let clone = argv(&["gh", "repo", "clone", &issue.repo, &checkout]);
  log.push(shell_join(&clone));
  if !dry {
    clone_once(graph, name, &checkout, &clone, env)?;
  }
  Ok(checkout)
}

/// Clone under a per-repository lock, and skip it when a concurrent launch
/// already cloned: launches of one batch share a missing repository.
fn clone_once(
  graph: &Graph,
  name: &str,
  checkout: &str,
  clone: &[String],
  env: &Env,
) -> Result<(), Failure> {
  let options = LockOptions {
    timeout: COMMAND_TIMEOUT + Duration::from_secs(10),
    stale: COMMAND_TIMEOUT + Duration::from_secs(5),
  };
  let lock = Path::new(&graph.state_dir).join(format!("clone-{name}"));
  if let Some(parent) = lock.parent() {
    std::fs::create_dir_all(parent)
      .map_err(|err| Failure::unknown(format!("cannot create {}: {err}", parent.display())))?;
  }
  with_lock(&lock, options, &mut Vec::new(), || {
    if Path::new(checkout).join(".git").exists() {
      return Ok(());
    }
    run_checked(clone, None, env, COMMAND_TIMEOUT).map(|_| ())
  })
}

fn default_branch(gh: Result<&Gh, String>, repo: &str) -> Result<String, Failure> {
  let failed =
    |err: String| Failure::unknown(format!("cannot read the default branch of {repo}: {err}"));
  let body: Value = gh
    .and_then(|gh| gh.get(&format!("/repos/{repo}")))
    .map_err(failed)?;
  body
    .get("default_branch")
    .and_then(Value::as_str)
    .filter(|branch| !branch.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| failed("no default_branch".to_owned()))
}

fn floor(env: &Env, name: &str, fallback: u64) -> u64 {
  env
    .get(name)
    .and_then(|value| value.trim().parse().ok())
    .unwrap_or(fallback)
}

/// Refuse a new worker while the GitHub budget is under its floor: every
/// worker adds babysit polling on the same token. An unreadable budget passes.
///
/// # Errors
/// The core or GraphQL budget is below its floor.
pub(crate) fn guard_budget(gh: Result<&Gh, String>, env: &Env) -> Result<(), Failure> {
  let Ok(body) = gh.and_then(|gh| gh.get::<Value>("/rate_limit")) else {
    return Ok(());
  };
  let resource = |name: &str| {
    let entry = body.pointer(&format!("/resources/{name}"))?;
    let number = |key: &str| entry.get(key).and_then(Value::as_u64);
    Some((number("remaining")?, number("limit")?, number("reset")?))
  };
  let (Some(core), Some(graphql)) = (resource("core"), resource("graphql")) else {
    return Ok(());
  };
  let floors = [
    ("core", core, floor(env, "EPIC_GH_CORE_FLOOR", 1000)),
    ("GraphQL", graphql, floor(env, "EPIC_GH_GRAPHQL_FLOOR", 500)),
  ];
  let Some((name, (remaining, limit, _), min)) = floors
    .iter()
    .find(|(_, (remaining, _, _), min)| remaining < min)
  else {
    return Ok(());
  };
  let reset = core.2.min(graphql.2);
  let at = toolu_state::time::iso_millis(UNIX_EPOCH + Duration::from_secs(reset));
  Err(Failure::refusal(format!(
    "GitHub {name} budget {remaining}/{limit} below floor {min}; resets by {at}. Launch later, or pass --force."
  )))
}

#[cfg(test)]
#[path = "tests/checkout_test.rs"]
mod tests;
