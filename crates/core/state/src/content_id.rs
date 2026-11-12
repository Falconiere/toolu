//! Working-tree blob ids for content-addressed reads.

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run};

/// Hash all paths in one Git process without worktree filters changing bytes.
/// A path Git's line-based stdin format cannot represent safely is uncached.
pub fn blob_ids(env: &Env, cwd: &Path, paths: &[String]) -> Option<Vec<String>> {
  if paths.is_empty() {
    return Some(Vec::new());
  }
  if paths
    .iter()
    .any(|path| path.contains(['\n', '\r', '"', '\\']))
  {
    return None;
  }
  let mut spec = Spec::new(["git", "hash-object", "--no-filters", "--stdin-paths"]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  spec.stdin = paths.join("\n").into_bytes();
  spec.stdin.push(b'\n');
  spec.max_output_bytes = paths.len().saturating_mul(65).max(1024);
  let output = run(&spec).ok()?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return None;
  }
  let ids: Vec<String> = output.stdout.lines().map(str::to_owned).collect();
  (ids.len() == paths.len()
    && ids
      .iter()
      .all(|id| matches!(id.len(), 40 | 64) && id.bytes().all(|byte| byte.is_ascii_hexdigit())))
  .then_some(ids)
}

#[cfg(test)]
#[path = "tests/content_id_test.rs"]
mod tests;
