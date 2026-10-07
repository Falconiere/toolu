//! Per-step freshness scopes (`packages/toolu-core/src/ledger/ledger-scope.ts`).
//! A step that declares `paths` is judged on the hash of `git diff
//! BASE...HEAD -- <paths>`, with the declaration hashed in: an edit outside
//! those paths leaves it fresh, and editing the declaration invalidates it.

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, run};

use super::check::command_substitution;
use super::jq::{JqError, each_optional, get, raw, string};

fn git(cwd: &Path, env: &Env, args: Vec<String>, stdin: Vec<u8>) -> Option<Vec<u8>> {
  let mut spec = Spec::new(["git".to_owned()].into_iter().chain(args));
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  spec.stdin = stdin;
  spec.max_output_bytes = usize::MAX;
  run(&spec)
    .ok()
    .filter(|out| out.exit_code == 0 && !out.truncated)
    .map(|out| out.stdout_bytes)
}

/// `pl_scope_sha BASE PATHS`: the hash of `<path>\0…` and the scoped diff, or
/// `None` when either git step fails (the caller then falls back to the branch hash).
pub fn scope_sha(base: &str, paths: &[String], cwd: &Path, env: &Env) -> Option<String> {
  if paths.is_empty() {
    return None;
  }
  let mut diff_args = vec![
    "diff".to_owned(),
    "--no-color".to_owned(),
    format!("{base}...HEAD"),
    "--".to_owned(),
  ];
  diff_args.extend(paths.iter().cloned());
  let diff = git(cwd, env, diff_args, Vec::new())?;
  let mut input: Vec<u8> = paths
    .iter()
    .flat_map(|path| path.bytes().chain([0]))
    .collect();
  input.extend(command_substitution(&diff));
  let hashed = git(
    cwd,
    env,
    vec!["hash-object".to_owned(), "--stdin".to_owned()],
    input,
  )?;
  let sha = String::from_utf8_lossy(&hashed).trim().to_owned();
  (!sha.is_empty()).then_some(sha)
}

/// `pl_scope_map STEPS BASE`: `{id: scope hash}` for every step that declares
/// non-empty `paths`. A step whose scope cannot be hashed is left out, with a
/// warning, so its fall-back to the whole branch diff is visible.
///
/// # Errors
/// [`JqError`] when a step is neither an object nor null.
pub fn scope_map(
  steps: &[Ordered],
  base: &str,
  cwd: &Path,
  env: &Env,
  warn: &mut dyn FnMut(String),
) -> Result<Ordered, JqError> {
  let mut out = Ordered::Object(Vec::new());
  for step in steps {
    let paths = get(step, "paths")?;
    if !matches!(paths, Ordered::Array(items) if !items.is_empty()) {
      continue;
    }
    let id = raw(get(step, "id")?);
    if id.is_empty() {
      continue;
    }
    let declared: Vec<String> = each_optional(paths).into_iter().map(raw).collect();
    match scope_sha(base, &declared, cwd, env) {
      Some(sha) => out.set(&id, string(&sha)),
      None => warn(format!(
        "plan-ledger: step {id} declares paths that could not be hashed; judging it on the whole branch diff"
      )),
    }
  }
  Ok(out)
}

#[cfg(test)]
#[path = "tests/scope_test.rs"]
mod tests;
