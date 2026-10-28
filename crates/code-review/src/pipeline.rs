//! Compose the committed-diff review stages after repository consent.

mod output;
mod state;

use crate::budget::Effort;
use crate::contract::{Omitted, Report};
use crate::memory;
use crate::prepass;
use crate::review::{self, Options, RunError};
use crate::rules;
use crate::settings;
use crate::shape::{self, Chunk, Shaped};
use clap::ArgMatches;
use output::{completed, failed};
use state::RunState;
use std::collections::BTreeSet;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::git::current_branch;

pub(crate) use state::dismiss;

/// Run one committed-diff review with the supplied endpoint configuration.
pub(crate) fn run(
  env: &Env,
  ctx: &Ctx,
  verb: &ArgMatches,
  options: &Options<'_>,
) -> Result<Report, String> {
  let roots = Roots::new(env.clone(), ctx.host);
  let root = project_root(&roots)?;
  let settings = settings::load(&roots, Some(&root));
  let effort = crate::cli::effort(verb)
    .or(settings.effort)
    .unwrap_or(Effort::Auto);
  if !settings.enabled {
    return Ok(RunError::NotEnabled.report(effort));
  }
  if env.get("OPENROUTER_API_KEY").is_none() {
    return Ok(RunError::Unconfigured("OPENROUTER_API_KEY is missing".to_owned()).report(effort));
  }
  let base = verb.get_one::<String>("base").map(String::as_str);
  let mut shaped = shape::shape(env, &root, base).map_err(|err| format!("shape: {err:?}"))?;
  let scoped = verb.get_many::<String>("path").is_some();
  if let Some(paths) = verb.get_many::<String>("path") {
    scope(&mut shaped, &paths.map(String::as_str).collect::<Vec<_>>());
  }
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
  review_and_persist(&state, &input, options, &prior, effort)
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
) -> Result<Report, String> {
  let (mut report, recap) = match review::run(state.env, input, options) {
    Ok(reviewed) => completed(state.shaped, state.prepass, prior, reviewed),
    Err(err) => failed(state.shaped, state.prepass, effort, &err),
  };
  state::persist(state, &mut report, &recap)?;
  Ok(report)
}

/// A path-scoped run sends only selected file chunks to the model.
fn scope(shaped: &mut Shaped, requested: &[&str]) {
  let selected: BTreeSet<&str> = requested.iter().copied().collect();
  shaped.chunks = shaped
    .files
    .iter()
    .enumerate()
    .filter(|(_, file)| selected.contains(file.path.as_str()))
    .map(|(index, file)| Chunk {
      files: vec![index],
      bytes: file.patch.len(),
    })
    .collect();
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
