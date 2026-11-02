use super::command;

#[test]
fn peer_commands_require_their_addressing_and_payload_arguments() {
  for args in [
    vec![
      "peers",
      "join",
      "--group",
      "team",
      "--machine",
      "one",
      "--agent",
      "codex",
      "--key-file",
      "key",
    ],
    vec![
      "peers",
      "--session",
      "alice",
      "send",
      "--to",
      "bob",
      "--generation",
      "gen",
      "--text",
      "hello",
    ],
    vec![
      "peers",
      "--session",
      "alice",
      "reply",
      "bob/m1",
      "--text",
      "hello",
    ],
    vec!["peers", "--session", "alice", "inbox", "--unread"],
    vec!["peers", "--session", "alice", "ack", "bob/m1"],
    vec!["peers", "--session", "alice", "status", "alice/m1"],
    vec!["peers", "--session", "alice", "list"],
    vec!["peers", "--session", "alice", "leave"],
  ] {
    assert!(command().try_get_matches_from(args).is_ok());
  }
  for args in [
    vec!["peers", "join"],
    vec!["peers", "send", "--to", "bob"],
    vec!["peers", "reply", "bob/m1"],
    vec!["peers", "ack"],
  ] {
    assert!(command().try_get_matches_from(args).is_err());
  }
}
