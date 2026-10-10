//! The v2 push-review file, preserving the old writer's field order and values.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use toolu_runtime::atomic::write_atomic;
use toolu_runtime::cli::Ctx;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, branch_slug, has_git};
use toolu_state::time::iso_seconds;

use crate::git;

const EMPTY_BLOB_SHA: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

/// Parsed CLI input whose JSON values retain their original order.
pub(crate) struct Input<'a> {
  pub(crate) findings_count: u64,
  pub(crate) reviewers: Ordered,
  pub(crate) findings: Ordered,
  pub(crate) repo: &'a str,
  pub(crate) branch: Option<&'a str>,
  pub(crate) reviewed_files: Option<&'a str>,
}

struct Prepared {
  branch: String,
  base: String,
  sha: String,
  files: Vec<String>,
  path: PathBuf,
}

fn prior_round(path: &Path, sha: &str) -> u64 {
  let Some(doc) = std::fs::read_to_string(path)
    .ok()
    .and_then(|text| Ordered::parse(&text).ok())
  else {
    return 0;
  };
  if doc.get("diff_sha") != Some(&Ordered::String(sha.to_owned())) {
    return 0;
  }
  let round = match doc.get("review_round") {
    Some(Ordered::String(text)) => text.clone(),
    Some(Ordered::Number(number)) => number.to_string(),
    _ => return 0,
  };
  if round.bytes().all(|byte| byte.is_ascii_digit()) {
    round.parse().unwrap_or(0)
  } else {
    0
  }
}

fn prepare(input: &Input<'_>, ctx: &Ctx, env: &Env) -> Result<Prepared, String> {
  if !has_git(env) {
    return Err("git required".to_owned());
  }
  let root = git::root(env, input.repo)?;
  let branch = git::branch(env, &root, input.branch)?;
  let base = env
    .get("PUSH_REVIEW_BASE")
    .map_or_else(|| base_branch(env, Some(&root), &root), str::to_owned);
  let sha = diff_sha(env, &root, &base).ok_or_else(|| format!("git diff {base}...HEAD failed"))?;
  if sha == EMPTY_BLOB_SHA {
    return Err(format!(
      "diff against {base} is empty; nothing to review yet"
    ));
  }
  let files = git::files(env, &root, &base, input.reviewed_files)?;
  let roots = Roots::new(env.clone(), ctx.host);
  let dir = env
    .get("STATE_DIR")
    .map(PathBuf::from)
    .or_else(|| {
      roots
        .project_state_dir("push-review", None, Some(&root))
        .ok()
        .flatten()
    })
    .unwrap_or(root);
  let path = dir.join(format!("{}.json", branch_slug(&branch)));
  Ok(Prepared {
    branch,
    base,
    sha,
    files,
    path,
  })
}

fn document(input: &Input<'_>, prepared: &Prepared) -> Ordered {
  let round = prior_round(&prepared.path, &prepared.sha).saturating_add(1);
  let field = |name: &str, value| (name.to_owned(), value);
  Ordered::Object(vec![
    field("version", Ordered::Number(2.into())),
    field("branch", Ordered::String(prepared.branch.clone())),
    field("diff_sha", Ordered::String(prepared.sha.clone())),
    field("base_branch", Ordered::String(prepared.base.clone())),
    field(
      "reviewed_at",
      Ordered::String(iso_seconds(SystemTime::now())),
    ),
    field("reviewers", input.reviewers.clone()),
    field(
      "findings_count",
      Ordered::Number(input.findings_count.into()),
    ),
    field("findings", input.findings.clone()),
    field("review_round", Ordered::Number(round.into())),
    field(
      "reviewed_files",
      Ordered::Array(
        prepared
          .files
          .iter()
          .cloned()
          .map(Ordered::String)
          .collect(),
      ),
    ),
  ])
}

/// Atomically write the review document and return its absolute file path.
pub(crate) fn write(input: &Input<'_>, ctx: &Ctx, env: &Env) -> Result<PathBuf, String> {
  let prepared = prepare(input, ctx, env)?;
  let body = format!("{}\n", document(input, &prepared).to_text(true));
  if !write_atomic(&prepared.path, &body) {
    return Err(format!(
      "cannot atomically write {}",
      prepared.path.display()
    ));
  }
  Ok(prepared.path)
}

#[cfg(test)]
#[path = "tests/state_test.rs"]
mod tests;
