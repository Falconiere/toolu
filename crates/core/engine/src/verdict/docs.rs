//! The verdict's docs gate (`docsGate` in `verdict-review.ts`): code changed
//! without a doc surface, and no attestation for this diff. It mirrors
//! docs-sync's surface classification and attestation lookup.

use std::path::Path;

use toolu_runtime::config::docs_sync::{DocsSyncKey, docs_sync_globs};
use toolu_runtime::config::load::{LoadedConfig, load};
use toolu_runtime::config::read::config_string;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, branch_slug};

use super::gates::{GateContext, field_or, gate, read_json};
use super::glob::matches_any;
use crate::ledger::parse::is_file;

/// Whether a diff with `changed` paths needs a doc it lacks.
fn needs_doc(changed: &[&str], surfaces: &[String], excludes: &[String], code: &[String]) -> bool {
  let has_doc = changed
    .iter()
    .any(|path| matches_any(path, surfaces) && !matches_any(path, excludes));
  let has_code = changed.iter().any(|path| matches_any(path, code));
  has_code && !has_doc
}

/// Whether the docs-sync attestation for this branch names `sha`.
fn attested(ctx: &GateContext<'_>, sha: &str) -> bool {
  let dir = match ctx.roots.env().get("DOCS_SYNC_STATE_DIR") {
    Some(dir) => dir.to_owned(),
    None => ctx
      .roots
      .project_state_dir("docs-sync", Some(&ctx.cwd), None)
      .ok()
      .flatten()
      .map(|dir| dir.display().to_string())
      .unwrap_or_default(),
  };
  let file = format!("{dir}/{}.json", branch_slug(&ctx.branch));
  if !is_file(Path::new(&file)) || sha.is_empty() {
    return false;
  }
  let doc = read_json(Path::new(&file)).unwrap_or(Ordered::Null);
  field_or(&doc, "diff_sha", "") == sha
}

/// The skips before any diff is read, in TypeScript's order.
fn early_skip(ctx: &GateContext<'_>, mode: &str, base: &str) -> Option<Ordered> {
  if mode == "off" {
    return Some(gate("skip", "docsSync.mode is off", Vec::new()));
  }
  if ctx.branch.is_empty() || ctx.branch == "HEAD" {
    return Some(gate("skip", "detached HEAD", Vec::new()));
  }
  if ctx
    .git(&["rev-parse", "--verify", "--quiet", base])
    .is_none()
  {
    return Some(gate(
      "skip",
      &format!("base branch '{base}' not found locally"),
      Vec::new(),
    ));
  }
  (ctx.branch == base).then(|| gate("skip", "current branch is the base branch", Vec::new()))
}

/// The gate's decision once the config is loaded.
fn decide(ctx: &GateContext<'_>, config: &LoadedConfig, base: &str) -> Ordered {
  let mode = config_string(
    config,
    "docsSync.mode",
    "advise",
    &["advise", "block", "off"],
  );
  if let Some(skip) = early_skip(ctx, &mode, base) {
    return skip;
  }
  let range = format!("{base}...HEAD");
  let names = ctx
    .git(&["diff", "--no-color", &range, "--name-only"])
    .unwrap_or_default();
  let changed: Vec<&str> = names.split('\n').filter(|path| !path.is_empty()).collect();
  if changed.is_empty() {
    return gate("skip", &format!("no diff against {base}"), Vec::new());
  }
  let globs = |key| docs_sync_globs(config, key);
  let surfaces = (
    globs(DocsSyncKey::Surfaces),
    globs(DocsSyncKey::SurfaceExcludes),
  );
  if !needs_doc(
    &changed,
    &surfaces.0,
    &surfaces.1,
    &globs(DocsSyncKey::CodeSurfaces),
  ) {
    return gate("pass", "doc surface in sync", Vec::new());
  }
  let sha = diff_sha(ctx.roots.env(), &ctx.root, base).unwrap_or_default();
  if attested(ctx, &sha) {
    return gate("pass", "doc change attested as not needed", Vec::new());
  }
  if mode == "block" {
    gate(
      "fail",
      "code changed without a doc update (docsSync.mode=block)",
      Vec::new(),
    )
  } else {
    gate("advise", "code changed without a doc update", Vec::new())
  }
}

/// `vd_gate_docs`; the config's warnings join the context's.
pub fn docs_gate(ctx: &mut GateContext<'_>) -> Ordered {
  let base = match ctx.roots.env().get("DOCS_SYNC_BASE") {
    Some(base) => base.to_owned(),
    None => base_branch(ctx.roots.env(), Some(&ctx.root), &ctx.cwd),
  };
  let config = load(ctx.roots, Some(&ctx.cwd));
  let decided = decide(ctx, &config, &base);
  ctx.warnings.extend(config.take_warnings());
  decided
}

#[cfg(test)]
#[path = "tests/docs_test.rs"]
mod tests;
