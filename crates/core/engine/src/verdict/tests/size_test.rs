//! The verdict's size gate over a real repository and config.

use toolu_runtime::json::ordered::Ordered;

use super::size_gate;
use crate::ledger::context::test_repo::Repo;

fn row(repo: &Repo, branch: &str) -> String {
  let opts = repo.opts();
  let gate = size_gate(&mut repo.gate(&opts.roots, branch, ""));
  match (gate.get("state"), gate.get("reason")) {
    (Some(Ordered::String(state)), Some(Ordered::String(reason))) => format!("{state}: {reason}"),
    _ => String::new(),
  }
}

fn config(repo: &Repo, mode: &str) {
  let body = format!(r#"{{"version":1,"gates":{{"prSize":{{"mode":"{mode}","maxLines":3}}}}}}"#);
  std::fs::write(repo.root.join(".home/toolu.config.json"), body).unwrap();
}

#[test]
fn the_row_reports_the_count_and_fails_only_where_the_push_would_stop() {
  let repo = Repo::new().unwrap();
  assert_eq!(
    row(&repo, "feat/x"),
    "pass: 1 of 300 code lines, 1 of 100 files against main"
  );
  repo
    .sh("seq 9 > big.rs && git add . && git commit -qm grow")
    .unwrap();
  config(&repo, "advise");
  assert_eq!(
    row(&repo, "feat/x"),
    "advise: 10 of 3 code lines, 2 of 100 files against main"
  );
  config(&repo, "block");
  assert_eq!(
    row(&repo, "feat/x"),
    "fail: 10 of 3 code lines, 2 of 100 files against main"
  );
  assert_eq!(
    row(&repo, "main"),
    "skip: current branch is the base branch"
  );
  repo.sh("git branch -q -m main trunk").unwrap();
  assert_eq!(row(&repo, "feat/x"), "fail: base 'main' not found locally");
  config(&repo, "off");
  assert_eq!(row(&repo, "feat/x"), "skip: prSize mode is off");
}
