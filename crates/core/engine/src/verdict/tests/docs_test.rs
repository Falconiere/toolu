//! The verdict's docs gate over real repositories and configs.

use toolu_runtime::json::ordered::Ordered;
use toolu_state::diff_sha::diff_sha;

use super::docs_gate;
use crate::ledger::context::test_repo::Repo;

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
    reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", ""))),
    "advise: code changed without a doc update"
  );
  config(&repo, "block");
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", ""))),
    "fail: code changed without a doc update (docsSync.mode=block)"
  );
  let sha = diff_sha(opts.env(), &repo.root, "main").unwrap();
  repo.sh(&format!("mkdir -p .claude/tmp/docs-sync && echo '{{\"diff_sha\":\"{sha}\"}}' > .claude/tmp/docs-sync/feat_x.json")).unwrap();
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", ""))),
    "pass: doc change attested as not needed"
  );
  repo
    .sh("echo doc > README.md && git add README.md && git commit -qm readme")
    .unwrap();
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", ""))),
    "pass: doc surface in sync"
  );
}

#[test]
fn the_gate_skips_off_detached_missing_base_and_empty_diffs() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  config(&repo, "off");
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", ""))),
    "skip: docsSync.mode is off"
  );
  config(&repo, "loud");
  let mut warned = repo.gate(&opts.roots, "HEAD", "");
  assert_eq!(reason(&docs_gate(&mut warned)), "skip: detached HEAD");
  assert_eq!(
    warned.warnings,
    ["docsSync.mode: 'loud' is not an allowed value (advise block off); using advise"]
  );
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "main", ""))),
    "skip: current branch is the base branch"
  );
  repo.sh("git checkout -q -b empty main").unwrap();
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "empty", ""))),
    "skip: no diff against main"
  );
  repo.sh("git branch -q -m main trunk").unwrap();
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "empty", ""))),
    "skip: base branch 'main' not found locally"
  );
}

#[test]
fn active_review_uses_the_hook_mode_resolver() {
  use toolu_protocol::event::HostEvent;
  use toolu_protocol::host::Host;
  use toolu_runtime::config::gate_mode::{GateMode, gate_mode};
  use toolu_runtime::config::load::load;

  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  crate::docs_review::register(opts.env(), Host::Claude);
  repo.sh("mkdir -p .claude").unwrap();
  let path = repo.root.join(".claude/toolu.config.json");
  for (body, expected, state) in [
    (
      r#"{"version":1,"gates":{"preset":"strict"}}"#,
      GateMode::Block,
      "fail:",
    ),
    (
      r#"{"version":1,"gates":{"preset":"strict","docsSync":{"mode":"advise"}}}"#,
      GateMode::Advise,
      "advise:",
    ),
    (
      r#"{"version":1,"gates":{"docsSync":{"mode":"off"}}}"#,
      GateMode::Off,
      "skip:",
    ),
  ] {
    std::fs::write(&path, body).unwrap();
    let config = load(&opts.roots, Some(&repo.root));
    assert_eq!(
      gate_mode(
        &config,
        "docsSync",
        Some(Host::Claude),
        Some(HostEvent::ShellPre)
      ),
      expected
    );
    assert!(reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", ""))).starts_with(state));
  }
}

#[test]
fn deselected_plugin_restores_legacy_verdict() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  crate::docs_review::register(opts.env(), toolu_protocol::host::Host::Claude);
  repo.sh("mkdir -p .claude").unwrap();
  let path = repo.root.join(".claude/toolu.config.json");
  let record = opts
    .roots
    .config_root()
    .join("plugins/installed_plugins.json");
  std::fs::create_dir_all(record.parent().unwrap()).unwrap();
  std::fs::write(record, r#"{"version":2,"plugins":{}}"#).unwrap();
  std::fs::write(&path, r#"{"version":1,"docsSync":{"mode":"block"}}"#).unwrap();
  assert_eq!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", ""))),
    "fail: code changed without a doc update (docsSync.mode=block)"
  );
}

#[test]
fn active_review_fails_closed_when_base_is_missing() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  crate::docs_review::register(opts.env(), toolu_protocol::host::Host::Claude);
  config(&repo, "block");
  repo.sh("git branch -m main missing-main").unwrap();
  assert!(
    reason(&docs_gate(&mut repo.gate(&opts.roots, "feat/x", "")))
      .starts_with("fail: documentation review required:")
  );
}

#[test]
fn active_verdict_reads_target_config_when_project_env_points_elsewhere() {
  let source = Repo::new().unwrap();
  let target = Repo::new().unwrap();
  config(&source, "off");
  config(&target, "block");
  let env = source
    .env()
    .with("TOOLU_PROJECT_DIR", &source.root.to_string_lossy());
  let roots = toolu_runtime::host::roots::Roots::new(env, Some(toolu_protocol::host::Host::Claude));
  crate::docs_review::register(roots.env(), roots.host());
  assert!(reason(&docs_gate(&mut target.gate(&roots, "feat/x", ""))).starts_with("fail:"));
}
