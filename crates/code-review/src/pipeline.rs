//! Compose the committed-diff review stages after repository consent.

mod output;
mod state;

use crate::budget::Effort;
use crate::contract::{ErrorKind, Omitted, Report, ReportError};
use crate::memory;
use crate::prepass;
use crate::review::{self, Options, RunError};
use crate::rules;
use crate::settings;
use crate::shape::{self, Shaped};
use clap::ArgMatches;
use output::{completed, failed};
use state::RunState;
use std::collections::BTreeSet;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::git::current_branch;

pub(crate) use state::{dismiss, settle};

/// Run one committed-diff review with the supplied endpoint configuration.
pub(crate) fn run(
  env: &Env,
  ctx: &Ctx,
  verb: &ArgMatches,
  options: &Options<'_>,
) -> Result<Report, String> {
  let roots = Roots::new(env.clone(), ctx.host);
  // Outside a repository there is no opt-in, so that run is still `not-enabled`.
  let root = project_root(&roots);
  let settings = settings::load(&roots, root.as_deref().ok());
  let effort = crate::cli::effort(verb)
    .or(settings.effort)
    .unwrap_or(Effort::Auto);
  if !settings.enabled {
    return Ok(RunError::NotEnabled.report(effort));
  }
  if env.get("OPENROUTER_API_KEY").is_none() {
    return Ok(RunError::Unconfigured("OPENROUTER_API_KEY is missing".to_owned()).report(effort));
  }
  let root = root?;
  let (shaped, scoped) = shaped(env, &roots, &root, verb)?;
  let prepass = prepass::run(env, &root, &shaped);
  let rules = rules::gather(env, &root, &shaped.merge_base, &shaped.gate_files)
    .map_err(|err| format!("rules: {err:?}"))?;
  let memory_path = branch_memory_path(env, &roots, &root)?;
  let prior = memory::load(&memory_path)?;
  let state = RunState {
    env,
    ctx,
    roots: &roots,
    root: &root,
    shaped: &shaped,
    prepass: &prepass,
    scoped,
    memory_path: &memory_path,
  };
  let input = review::Input {
    root: &root,
    shaped: &shaped,
    rules: &rules,
    prepass: &prepass,
    memory: &prior.memory,
    settings: &settings,
    flag: crate::cli::effort(verb),
  };
  Ok(review_and_persist(&state, &input, options, &prior, effort))
}

/// The committed diff, narrowed to `--path` selections; `true` when scoped.
fn shaped(
  env: &Env,
  roots: &Roots,
  root: &std::path::Path,
  verb: &ArgMatches,
) -> Result<(Shaped, bool), String> {
  let base = verb.get_one::<String>("base").map(String::as_str);
  let mut shaped = review_shape(env, roots, root, base)?;
  let paths = verb.get_many::<String>("path");
  let scoped = paths.is_some();
  if let Some(paths) = paths {
    scope(&mut shaped, &paths.map(String::as_str).collect::<Vec<_>>());
  }
  Ok((shaped, scoped))
}

fn review_shape(
  env: &Env,
  roots: &Roots,
  root: &std::path::Path,
  base: Option<&str>,
) -> Result<Shaped, String> {
  let mut shaped = shape::shape(env, root, base).map_err(|err| format!("shape: {err:?}"))?;
  if shaped.uncommitted {
    let state_root = roots
      .project_state_root(None, Some(root))
      .ok_or("state root unavailable")?;
    shaped.uncommitted =
      shape::dirty_except_state(env, root, &state_root).map_err(|err| format!("shape: {err:?}"))?;
  }
  Ok(shaped)
}

fn project_root(roots: &Roots) -> Result<std::path::PathBuf, String> {
  roots
    .project_root(None)
    .ok_or_else(|| "not inside a Git repository".to_owned())
}

fn branch_memory_path(
  env: &Env,
  roots: &Roots,
  root: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
  let branch = current_branch(env, root);
  if branch.is_empty() || branch == "HEAD" {
    return Err("code-review requires an attached branch".to_owned());
  }
  memory::path(roots, root, &branch)
}

fn review_and_persist(
  state: &RunState<'_>,
  input: &review::Input<'_>,
  options: &Options<'_>,
  prior: &memory::BranchState,
  effort: Effort,
) -> Report {
  let (mut report, recap) = match review::run(state.env, input, options) {
    Ok(reviewed) => completed(state.shaped, state.prepass, prior, reviewed),
    // Nothing was sent, so nothing is recorded: no pass, usage row or gate round.
    Err(err @ (RunError::NotEnabled | RunError::Unconfigured(_))) => return err.report(effort),
    Err(err) => failed(state.shaped, state.prepass, effort, &err),
  };
  if let Err(message) = state::persist(state, &mut report, &recap) {
    // An outage or deadline keeps its kind: agents branch on it.
    match &mut report.error {
      Some(error) => error.message = format!("{}; state: {message}", error.message),
      None => {
        report.error = Some(ReportError {
          kind: ErrorKind::State,
          message,
        });
      }
    }
  }
  report
}

/// A path-scoped run sends only selected file chunks to the model.
fn scope(shaped: &mut Shaped, requested: &[&str]) {
  let selected: BTreeSet<&str> = requested.iter().copied().collect();
  let files = &shaped.files;
  let wanted = |index: &usize| {
    files
      .get(*index)
      .is_some_and(|file| selected.contains(file.path.as_str()))
  };
  // Narrowing the shaper's chunks keeps its packing, so a scope costs no extra calls.
  for chunk in &mut shaped.chunks {
    chunk.files.retain(wanted);
    chunk.bytes = chunk
      .files
      .iter()
      .filter_map(|index| files.get(*index))
      .map(|file| file.patch.len())
      .sum();
  }
  shaped.chunks.retain(|chunk| !chunk.files.is_empty());
  let omitted: Vec<String> = shaped
    .coverage
    .reviewed
    .iter()
    .filter(|path| !selected.contains(path.as_str()))
    .cloned()
    .collect();
  shaped
    .coverage
    .reviewed
    .retain(|path| selected.contains(path.as_str()));
  shaped
    .coverage
    .unreviewed
    .extend(omitted.into_iter().map(|path| Omitted {
      path,
      reason: "outside requested paths".to_owned(),
    }));
}

#[cfg(test)]
#[path = "tests/pipeline_test.rs"]
mod tests;
