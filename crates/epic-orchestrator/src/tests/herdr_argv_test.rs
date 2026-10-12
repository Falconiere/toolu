use toolu_runtime::env::Env;

use super::sample_calls;

#[test]
fn argv_names_every_control_verb() {
  let env = Env::from_pairs([("HOME", "/tmp"), ("TOOLU_EPIC_HERDR_SESSION", "toolu446")]);
  let calls = sample_calls();
  let joined: Vec<String> = calls.iter().map(|call| call.argv(&env).join(" ")).collect();
  assert!(joined[0].starts_with("herdr --session toolu446 agent start a --kind claude"));
  assert!(joined[0].contains("--pane w1:p1 --timeout 1000 -- status"));
  assert_eq!(joined[1], "herdr --session toolu446 agent prompt a STATUS?");
  assert!(joined[2].contains("agent read a --source recent-unwrapped"));
  assert!(joined[3].contains("worktree create --cwd /work --branch feat --base main"));
  assert!(joined[3].contains("--workspace w1"));
  assert!(joined[4].ends_with("worktree remove --workspace w1 --force"));
  assert_eq!(
    calls[5].argv(&env)[3..],
    [
      "notification",
      "show",
      "toolu epic: a needs-human",
      "--body",
      "which one?",
      "--sound",
      "request"
    ]
  );
}

#[test]
fn queries_carry_their_flags_and_no_session_without_one() {
  use crate::herdr::Call;
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let open = Call::WorktreeOpen {
    cwd: "/r".into(),
    path: "/r/t".into(),
    label: "k".into(),
  };
  assert_eq!(
    open.argv(&env).join(" "),
    "herdr worktree open --cwd /r --path /r/t --label k --no-focus"
  );
  let start = Call::Start {
    name: "k".into(),
    kind: "codex".into(),
    pane_id: "w1:p1".into(),
    args: Vec::new(),
    timeout_ms: None,
  };
  assert_eq!(
    start.argv(&env).join(" "),
    "herdr agent start k --kind codex --pane w1:p1"
  );
  let create = Call::Create {
    cwd: Some("/r".into()),
    branch: None,
    base: None,
    path: None,
    label: None,
    workspace: None,
  };
  assert_eq!(
    create.argv(&env).join(" "),
    "herdr worktree create --cwd /r --no-focus"
  );
  let keys = Call::SendKeys {
    target: "k".into(),
    keys: vec!["ctrl+c".into(), "esc".into()],
  };
  assert_eq!(
    keys.argv(&env).join(" "),
    "herdr agent send-keys k ctrl+c esc"
  );
}
