use serde_json::json;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{Session, Tool};
use toolu_protocol::text::Text;

use super::*;
use crate::gate::Gate;
use crate::ledger::context::test_repo::Repo;

fn config(data: &serde_json::Value) -> LoadedConfig {
  LoadedConfig::from_data(data.as_object().cloned().expect("object"), Host::Claude)
}

/// `feat/x` grown by `lines` reviewable lines and a 500-line lockfile.
fn grown(lines: usize) -> Repo {
  let repo = Repo::new().expect("repo");
  let script =
    format!("seq {lines} > big.rs && seq 500 > Cargo.lock && git add . && git commit -qm grow");
  repo.sh(&script).expect("grow");
  repo
}

fn push(repo: &Repo, env: &Env) -> Decision {
  let text = |value: &str| Text::new(value).expect("text");
  let command = "git push origin HEAD:feat/x";
  let event = NormalizedEvent::ShellPre {
    session: Session {
      session_id: text("s"),
      cwd: text("cwd"),
      project_root: text("root"),
      worktree: text("tree"),
    },
    tool: Tool {
      call_id: text("c"),
      name: text("Bash"),
      input: serde_json::Map::new(),
    },
    command: text(command),
  };
  let raw = json!({"tool_input": {"command": command}});
  let ctx = RuleContext {
    host: Host::Claude,
    env,
    config_root: &repo.root,
    project_root: &repo.root,
    cwd: Some(&repo.root),
    plugin_root: None,
    raw: raw.as_object().expect("object"),
    edit: None,
  };
  PR_SIZE.run(&event, &ctx).expect("decision")
}

#[test]
fn code_lines_skip_tests_docs_and_generated_files_but_every_file_is_counted() {
  let numstat = "10\t90\tsrc/lib.rs\n500\t0\tCargo.lock\n-\t-\tlogo.png\n7\t0\tdocs/cli/x.md\n3\t1\tsrc/{old => new}/gate.rs\n40\t0\ttestdata/big.json\n";
  assert_eq!(counted(numstat, &config(&json!({}))), (13, 6));
  let all = config(&json!({"gates": {"prSize": {"exclude": []}}}));
  assert_eq!(counted(numstat, &all), (560, 6));
  let locks = config(&json!({"gates": {"prSize": {"exclude": ["*.lock"]}}}));
  assert_eq!(counted(numstat, &locks), (60, 6));
  let also = config(&json!({"gates": {"prSize": {"alsoExclude": ["src/lib.rs"]}}}));
  assert_eq!(counted(numstat, &also), (3, 6));
}

#[test]
fn a_branch_is_measured_against_both_configured_budgets() {
  let repo = grown(9);
  let env = repo.env();
  let size = branch_size(&repo.root, &env, &config(&json!({})))
    .expect("base")
    .expect("size");
  assert_eq!(
    size.summary(),
    "10 of 300 code lines, 3 of 100 files against main"
  );
  assert!(!size.over());
  for tight in [json!({"maxLines": 9}), json!({"maxFiles": 2})] {
    let config = config(&json!({"gates": {"prSize": tight}}));
    assert!(
      branch_size(&repo.root, &env, &config)
        .expect("base")
        .expect("size")
        .over()
    );
  }
  let wrong = config(&json!({"gates": {"prSize": {"maxLines": 0, "maxFiles": "9"}}}));
  let size = branch_size(&repo.root, &env, &wrong)
    .expect("base")
    .expect("size");
  assert_eq!((size.max, size.max_files), (300, 100));
  assert_eq!(
    wrong.take_warnings(),
    [
      "gates.prSize.maxLines: 0 is not a positive integer; using 300",
      "gates.prSize.maxFiles: \"9\" is not a positive integer; using 100",
    ]
  );
}

#[test]
fn the_remote_base_wins_over_a_lagging_local_base() {
  let repo = grown(9);
  repo
    .sh("git update-ref refs/remotes/origin/main HEAD~1")
    .expect("origin");
  let unpinned = repo.env().with("PUSH_REVIEW_BASE", "");
  let size = branch_size(&repo.root, &unpinned, &config(&json!({})))
    .expect("base")
    .expect("size");
  assert_eq!((size.added, size.base.as_str()), (9, "origin/main"));
  let pinned = branch_size(&repo.root, &repo.env(), &config(&json!({})))
    .expect("base")
    .expect("size");
  assert_eq!((pinned.added, pinned.base.as_str()), (10, "main"));
}

#[test]
fn an_over_budget_push_advises_by_default_and_pins_honor_the_mode() {
  let repo = grown(350);
  let env = repo.env();
  let Decision::Advisory { message } = push(&repo, &env) else {
    panic!("the balanced preset advises");
  };
  assert!(
    message
      .as_str()
      .contains("351 of 300 code lines, 3 of 100 files against main"),
    "{message:?}"
  );
  assert!(
    message.as_str().contains("does not block the push"),
    "{message:?}"
  );
  let file = repo.root.join(".home/toolu.config.json");
  let set = |body: &str| std::fs::write(&file, body).expect("config");
  set(r#"{"version":1,"gates":{"prSize":{"mode":"block"}}}"#);
  let Decision::Deny { reason } = push(&repo, &env) else {
    panic!("block mode denies");
  };
  assert!(reason.as_str().contains("Push denied"), "{reason:?}");
  set(r#"{"version":1,"gates":{"prSize":{"maxLines":351}}}"#);
  assert_eq!(push(&repo, &env), Decision::Allow);
  set(r#"{"version":1,"gates":{"prSize":{"mode":"block"}}}"#);
  let Decision::Deny { reason } = push(&repo, &env.clone().with("PUSH_REVIEW_BASE", "gone")) else {
    panic!("an unmeasured branch is denied in block mode");
  };
  assert!(
    reason.as_str().contains("base 'gone' not found locally"),
    "{reason:?}"
  );
  set(r#"{"version":1}"#);
  assert!(matches!(
    push(&repo, &env.clone().with("PUSH_REVIEW_BASE", "gone")),
    Decision::Advisory { .. }
  ));
  repo
    .sh("git branch -q -D main && git branch -q -m main")
    .expect("rename");
  assert_eq!(push(&repo, &env), Decision::Allow);
}
