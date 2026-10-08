//! Nudge a code-only push until a doc changes or the exact diff is attested.

use std::path::{Path, PathBuf};

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::docs_sync::{DocsSyncKey, docs_sync_globs};
use toolu_runtime::config::gate_mode::GateMode;
use toolu_runtime::config::load::LoadedConfig;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::ctx::StateCtx;
use toolu_state::diff_sha::diff_sha;
use toolu_state::git::{base_branch, branch_slug};
use toolu_state::telemetry::{TelemetryEvent, telemetry_append};

use super::push_target::{PushTarget, git_at, push_target, ref_exists};
use super::{decided, gate_config, pre_mode};
use crate::ledger::parse::is_file;
use crate::verdict::gates::{field_or, read_json};
use crate::verdict::glob::matches_any;

/// The docs-sync built-in.
pub(crate) struct DocsSync;
/// Its singleton in the pre-tool table.
pub(crate) static DOCS_SYNC: DocsSync = DocsSync;

struct Diff {
  root: PathBuf,
  branch: String,
  sha: String,
  changed: Vec<String>,
}

fn diff_target(event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Option<Diff> {
  let PushTarget { root, branch } = push_target(event, ctx)?;
  if branch.is_empty() || branch == "HEAD" {
    return None;
  }
  let base = ctx
    .env
    .get("DOCS_SYNC_BASE")
    .map_or_else(|| base_branch(ctx.env, Some(&root), &root), str::to_owned);
  if !ref_exists(&root, &base, ctx.env) || branch == base {
    return None;
  }
  let names = git_at(
    &root,
    &["diff", "--name-only", &format!("{base}...HEAD")],
    ctx.env,
  )
  .unwrap_or_default();
  let changed: Vec<String> = names
    .lines()
    .filter(|path| !path.is_empty())
    .map(str::to_owned)
    .collect();
  if changed.is_empty() {
    return None;
  }
  let sha = diff_sha(ctx.env, &root, &base).unwrap_or_default();
  Some(Diff {
    root,
    branch,
    sha,
    changed,
  })
}

fn needs_doc(changed: &[String], config: &LoadedConfig) -> bool {
  let surfaces = docs_sync_globs(config, DocsSyncKey::Surfaces);
  let excludes = docs_sync_globs(config, DocsSyncKey::SurfaceExcludes);
  let code = docs_sync_globs(config, DocsSyncKey::CodeSurfaces);
  let has_doc = changed
    .iter()
    .any(|path| matches_any(path, &surfaces) && !matches_any(path, &excludes));
  let has_code = changed.iter().any(|path| matches_any(path, &code));
  has_code && !has_doc
}

fn attested(file: &Path, sha: &str) -> Option<String> {
  if !is_file(file) || sha.is_empty() {
    return None;
  }
  let doc = read_json(file).unwrap_or(toolu_runtime::json::ordered::Ordered::Null);
  (field_or(&doc, "diff_sha", "") == sha).then(|| field_or(&doc, "decision", ""))
}

fn consequence(mode: GateMode) -> &'static str {
  match mode {
    GateMode::Block => "Push denied until a doc is updated or a valid attestation is written.",
    GateMode::Ask => "Approve to push anyway, or update the doc first.",
    GateMode::Advise => "Advisory only — this does not block the push.",
    GateMode::Off => "",
  }
}

fn emit(
  diff: &Diff,
  mode: GateMode,
  roots: Roots,
  warnings: &mut Vec<String>,
) -> Result<Decision, String> {
  let dir = roots.env().get("DOCS_SYNC_STATE_DIR").map_or_else(
    || {
      roots
        .project_state_dir("docs-sync", None, Some(&diff.root))
        .ok()
        .flatten()
        .unwrap_or_default()
        .display()
        .to_string()
    },
    str::to_owned,
  );
  let file = PathBuf::from(format!("{dir}/{}.json", branch_slug(&diff.branch)));
  let mut state = StateCtx::new(roots);
  let result = if let Some(decision) = attested(&file, &diff.sha) {
    let _written = telemetry_append(
      &mut state,
      &diff.root,
      &TelemetryEvent::DocsAttested { decision },
    );
    Ok(Decision::Allow)
  } else {
    let _written = telemetry_append(&mut state, &diff.root, &TelemetryEvent::DocsNudge);
    decided(
      mode,
      format!(
        "docs-sync: this branch changes code but no documentation surface (README / docs/*.md / SKILL.md). Update the doc that describes this behavior, OR attest none is needed by writing {} with {{ \"version\": 1, \"diff_sha\": \"{}\", \"decision\": \"not-needed\", \"note\": \"why\" }}. {}",
        file.display(),
        diff.sha,
        consequence(mode)
      ),
    )
  };
  warnings.append(&mut state.warnings);
  result
}

fn evaluate(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  warnings: &mut Vec<String>,
) -> Result<Decision, String> {
  let Some(diff) = diff_target(event, ctx) else {
    return Ok(Decision::Allow);
  };
  let config = gate_config(ctx);
  if !needs_doc(&diff.changed, &config) {
    return Ok(Decision::Allow);
  }
  let mode = pre_mode(&config, "docsSync", ctx, true);
  if mode == GateMode::Off {
    return Ok(Decision::Allow);
  }
  emit(
    &diff,
    mode,
    Roots::new(ctx.env.clone(), Some(ctx.host)),
    warnings,
  )
}

impl_pre_gate!(DocsSync, "docs-sync", evaluate);

#[cfg(test)]
#[path = "tests/docs_sync_test.rs"]
mod tests;
