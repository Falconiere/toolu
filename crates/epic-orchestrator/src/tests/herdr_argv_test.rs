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
}
