//! What every verdict gate shares (`packages/toolu-core/src/ledger/verdict-gates.ts`),
//! and the quality gate. Each gate is a read-only mirror of its enforcement
//! point: it never writes state or runs a check, and it reports one object,
//! `{state, reason, ...extra}`.

use std::path::{Path, PathBuf};

use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::process::{Spec, run};

use crate::ledger::io::state_root_for;
use crate::ledger::jq::{JqError, alt, get, parse_json, raw, string};

/// What every gate reads.
#[derive(Debug)]
pub struct GateContext<'a> {
  /// The environment and host.
  pub roots: &'a Roots,
  /// The repository root.
  pub root: PathBuf,
  /// The current branch.
  pub branch: String,
  /// The base branch.
  pub base: String,
  /// The branch hash, or `""`.
  pub cur: String,
  /// The invocation directory.
  pub cwd: PathBuf,
  /// Config warnings, for stderr.
  pub warnings: Vec<String>,
}

/// `{state, reason, ...extra}`.
pub fn gate(state: &str, reason: &str, extra: Vec<(String, Ordered)>) -> Ordered {
  let mut entries = vec![
    ("state".to_owned(), string(state)),
    ("reason".to_owned(), string(reason)),
  ];
  entries.extend(extra);
  Ordered::Object(entries)
}

impl GateContext<'_> {
  /// `git -C <root> <args>` stdout, or `None` on a non-zero exit.
  pub fn git(&self, args: &[&str]) -> Option<String> {
    let mut spec = Spec::new(["git", "-C"]);
    spec.argv.push(self.root.display().to_string());
    spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
    spec.env = Some(self.roots.env().clone());
    spec.max_output_bytes = usize::MAX;
    run(&spec)
      .ok()
      .filter(|out| out.exit_code == 0)
      .map(|out| out.stdout)
  }

  /// `${OVERRIDE:-$(toolu_project_state_dir NAME ROOT)}`.
  pub fn state_dir(&self, name: &str, override_var: &str) -> PathBuf {
    match self.roots.env().get(override_var) {
      Some(dir) => PathBuf::from(dir),
      None => state_root_for(&self.root, self.roots).join(name),
    }
  }
}

/// A JSON file as jq reads it, or `None` when absent or not JSON.
pub fn read_json(file: &Path) -> Option<Ordered> {
  let bytes = std::fs::read(file).ok()?;
  parse_json(&String::from_utf8_lossy(&bytes))
}

/// `jq -r` of a computed value, where a jq error reads as `fallback`.
pub fn raw_or(value: Result<&Ordered, JqError>, fallback: &str) -> String {
  value.map_or_else(|_| fallback.to_owned(), raw)
}

/// `jq -r '.key // fallback'` over `doc`, `fallback` on a jq error.
pub fn field_or(doc: &Ordered, key: &str, fallback: &str) -> String {
  let fallback_value = string(fallback);
  raw_or(
    get(doc, key).map(|value| alt(value, &fallback_value)),
    fallback,
  )
}

/// `vd_gate_quality`: a linked worktree skips; otherwise the gate file's status decides.
pub fn quality_gate(ctx: &GateContext<'_>) -> Ordered {
  // `--path-format` needs Git 2.31+; on older Git both dirs read empty and the check skips alike.
  let dir = |flag: &str| {
    let text = ctx
      .git(&["rev-parse", "--path-format=absolute", flag])
      .unwrap_or_default();
    let trimmed = text.trim();
    trimmed.strip_suffix('/').unwrap_or(trimmed).to_owned()
  };
  let (git_dir, common_dir) = (dir("--git-dir"), dir("--git-common-dir"));
  if !git_dir.is_empty() && !common_dir.is_empty() && git_dir != common_dir {
    return gate("skip", "gate disabled in linked worktree", Vec::new());
  }
  let file = state_root_for(&ctx.root, ctx.roots).join("quality-gate-status.json");
  if !std::fs::metadata(&file).is_ok_and(|meta| meta.is_file()) {
    return gate("pass", "no quality-gate failure recorded", Vec::new());
  }
  let doc = read_json(&file).unwrap_or(Ordered::Null);
  if field_or(&doc, "status", "") != "failing" {
    return gate("pass", "quality gate passing", Vec::new());
  }
  let reason = field_or(&doc, "reason", "Quality gate failing");
  gate("fail", reason.trim_end_matches('\n'), Vec::new())
}

#[cfg(test)]
#[path = "tests/gates_test.rs"]
mod tests;
