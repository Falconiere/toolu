//! The verdict's docs gate over real repositories and configs.

use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;

use super::docs_gate;
use crate::ledger::context::test_repo::Repo;
use crate::verdict::gates::GateContext;

fn ctx<'a>(
  repo: &Repo,
  roots: &'a toolu_runtime::host::roots::Roots,
  branch: &str,
) -> GateContext<'a> {
  GateContext {
    roots,
    root: repo.root.clone(),
    branch: branch.to_owned(),
    base: "main".to_owned(),
    cur: String::new(),
    cwd: repo.root.clone(),
    warnings: Vec::new(),
  }
}

fn reason(gate: &Ordered) -> String {
  match (gate.get("state"), gate.get("reason")) {
    (Some(Ordered::String(state)), Some(Ordered::String(reason))) => format!("{state}: {reason}"),
    _ => String::new(),
  }
}

fn config(repo: &Repo, mode: &str) {
  repo.sh("mkdir -p .claude").unwrap();
  let body = format!(r#"{{"version":1,"docsSync":{{"mode":"{mode}"}}}}"#);
  std::fs::write(repo.root.join(".claude/toolu.config.json"), body).unwrap();
}

#[test]
fn code_without_docs_advises_or_blocks_until_attested() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "feat/x"))),
    "advise: code changed without a doc update"
  );
  config(&repo, "block");
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "feat/x"))),
    "fail: code changed without a doc update (docsSync.mode=block)"
  );
  let sha = diff_sha(opts.env(), &repo.root, "main").unwrap();
  repo.sh(&format!("mkdir -p .claude/tmp/docs-sync && echo '{{\"diff_sha\":\"{sha}\"}}' > .claude/tmp/docs-sync/feat_x.json")).unwrap();
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "feat/x"))),
    "pass: doc change attested as not needed"
  );
  repo
    .sh("echo doc > README.md && git add README.md && git commit -qm readme")
    .unwrap();
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "feat/x"))),
    "pass: doc surface in sync"
  );
}

#[test]
fn the_gate_skips_off_detached_missing_base_and_empty_diffs() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  config(&repo, "off");
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "feat/x"))),
    "skip: docsSync.mode is off"
  );
  config(&repo, "loud");
  let mut warned = ctx(&repo, &opts.roots, "HEAD");
  assert_eq!(reason(&docs_gate(&mut warned)), "skip: detached HEAD");
  assert_eq!(
    warned.warnings,
    ["docsSync.mode: 'loud' is not an allowed value (advise block off); using advise"]
  );
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "main"))),
    "skip: current branch is the base branch"
  );
  repo.sh("git checkout -q -b empty main").unwrap();
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "empty"))),
    "skip: no diff against main"
  );
  repo.sh("git branch -q -m main trunk").unwrap();
  assert_eq!(
    reason(&docs_gate(&mut ctx(&repo, &opts.roots, "empty"))),
    "skip: base branch 'main' not found locally"
  );
}
