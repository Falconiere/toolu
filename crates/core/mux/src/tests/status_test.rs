use super::{AgentStatus, derive};

#[test]
fn a_known_prompt_is_blocked_regardless_of_foreground() {
  assert_eq!(
    derive("claude", "Do you want to proceed?\n❯ 1. Yes"),
    AgentStatus::Blocked
  );
  assert_eq!(
    derive("zsh", "Do you want to make this edit to foo.rs?"),
    AgentStatus::Blocked
  );
}

#[test]
fn a_shell_foreground_with_a_clean_screen_is_done() {
  assert_eq!(derive("zsh", "$ "), AgentStatus::Done);
  assert_eq!(derive("bash", "user@host:~$ "), AgentStatus::Done);
}

#[test]
fn a_host_cli_foreground_is_working() {
  assert_eq!(derive("claude", "Thinking…"), AgentStatus::Working);
  assert_eq!(derive("node", "codex> "), AgentStatus::Working);
}

#[test]
fn an_empty_foreground_is_unknown() {
  assert_eq!(derive("", ""), AgentStatus::Unknown);
}

#[test]
fn the_status_word_matches_herdr() {
  assert_eq!(AgentStatus::Idle.as_str(), "idle");
  assert_eq!(AgentStatus::Working.as_str(), "working");
  assert_eq!(AgentStatus::Blocked.as_str(), "blocked");
  assert_eq!(AgentStatus::Done.as_str(), "done");
  assert_eq!(AgentStatus::Unknown.as_str(), "unknown");
}
