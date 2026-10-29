use crate::test_support::World;

use super::TmuxError;

#[test]
fn a_session_is_created_listed_and_killed() {
  let world = World::new();
  let client = &world.client;
  assert!(!client.session_exists("toolu-test"));
  client.create_session("toolu-test", &world.repo).unwrap();
  assert!(client.session_exists("toolu-test"));
  assert!(
    client
      .list_sessions()
      .unwrap()
      .contains(&"toolu-test".to_owned())
  );
  client.kill_session("toolu-test").unwrap();
  assert!(!client.session_exists("toolu-test"));
}

#[test]
fn creating_an_existing_session_fails() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-dup", &world.repo).unwrap();
  let err = client.create_session("toolu-dup", &world.repo).unwrap_err();
  assert_eq!(err.code, "workspace_exists");
}

#[test]
fn literal_text_is_sent_and_captured() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-echo", &world.repo).unwrap();
  let pane = client.first_pane("toolu-echo").unwrap();
  client.send_literal(&pane, "echo hello-world").unwrap();
  // Give the shell a moment to run the command.
  std::thread::sleep(std::time::Duration::from_millis(300));
  let text = client.capture(&pane, 5).unwrap();
  assert!(text.contains("hello-world"), "{text}");
}

#[test]
fn the_first_pane_has_a_pid() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-pid", &world.repo).unwrap();
  let pane = client.first_pane("toolu-pid").unwrap();
  let pid = client.pane_pid(&pane).unwrap();
  assert!(pid > 0);
}

#[test]
fn named_keys_reach_the_pane() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-keys", &world.repo).unwrap();
  let pane = client.first_pane("toolu-keys").unwrap();
  client.send_literal(&pane, "cat").unwrap();
  std::thread::sleep(std::time::Duration::from_millis(200));
  client.send_keys(&pane, &["C-c"]).unwrap();
}

#[test]
fn a_missing_pane_fails_lookups() {
  let world = World::new();
  let client = &world.client;
  // Start the server with an unrelated session so the socket exists.
  client.create_session("toolu-anchor", &world.repo).unwrap();
  let err: TmuxError = client.pane_pid("%999999").unwrap_err();
  assert_eq!(err.code, "pane_not_found");
}

#[test]
fn a_missing_session_fails_the_first_pane_lookup() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-anchor", &world.repo).unwrap();
  let err = client.first_pane("toolu-nope").unwrap_err();
  assert_eq!(err.code, "pane_not_found");
}

#[test]
fn killing_a_missing_session_is_a_no_op() {
  let world = World::new();
  assert!(world.client.kill_session("toolu-ghost").is_ok());
}

#[test]
fn listing_sessions_on_an_empty_server_is_empty_or_erroring() {
  let world = World::new();
  // No session has been created yet, so the server may not even be running.
  let sessions = world.client.list_sessions().unwrap_or_default();
  assert_eq!(sessions, Vec::<String>::new());
}

#[test]
fn a_status_message_is_sent_to_the_session() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-notify", &world.repo).unwrap();
  client
    .display_message("toolu-notify", "hello from the test")
    .unwrap();
}

#[test]
fn an_unknown_tmux_subcommand_fails() {
  let world = World::new();
  let err = world.client.run(&["not-a-real-subcommand"]).unwrap_err();
  assert_eq!(err.code, "tmux");
}

#[test]
fn start_agent_types_the_argv_into_the_pane() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-start", &world.repo).unwrap();
  let pane = client.first_pane("toolu-start").unwrap();
  client.start_agent(&pane, &["echo", "started"]).unwrap();
  std::thread::sleep(std::time::Duration::from_millis(300));
  let text = client.capture(&pane, 5).unwrap();
  assert!(text.contains("started"), "{text}");
}

#[test]
fn starting_an_empty_command_fails() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-empty", &world.repo).unwrap();
  let pane = client.first_pane("toolu-empty").unwrap();
  let err = client.start_agent(&pane, &[]).unwrap_err();
  assert_eq!(err.code, "agent_not_ready");
}

#[test]
fn prompt_sends_text_and_a_shell_returns_to_done() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-prompt", &world.repo).unwrap();
  let pane = client.first_pane("toolu-prompt").unwrap();
  client.prompt(&pane, "true").unwrap();
  let status = client
    .wait(
      &pane,
      &[crate::status::AgentStatus::Done],
      std::time::Duration::from_secs(5),
    )
    .unwrap();
  assert_eq!(status.as_str(), "done");
}

#[test]
fn waiting_for_an_unreachable_status_times_out() {
  let world = World::new();
  let client = &world.client;
  client.create_session("toolu-wait", &world.repo).unwrap();
  let pane = client.first_pane("toolu-wait").unwrap();
  let err = client
    .wait(
      &pane,
      &[crate::status::AgentStatus::Blocked],
      std::time::Duration::from_millis(300),
    )
    .unwrap_err();
  assert_eq!(err.code, "timeout");
}
