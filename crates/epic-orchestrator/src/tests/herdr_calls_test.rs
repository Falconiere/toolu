use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use super::{CallError, request, result_of};
use crate::herdr::{Call, Client};
use crate::paths::Paths;

const SCHEMA: &str = include_str!("../../../../fixtures/epic/herdr-calls.json");

fn every_call() -> Vec<Call> {
  let mut calls = crate::herdr_argv::sample_calls().to_vec();
  calls.extend([
    Call::WorkspaceList,
    Call::WorktreeList { cwd: "/w".into() },
    Call::WorktreeOpen {
      cwd: "/w".into(),
      path: "/w/t".into(),
      label: "k".into(),
    },
    Call::PaneList {
      workspace_id: "w1".into(),
    },
    Call::PaneInfo {
      pane_id: "w1:p1".into(),
    },
    Call::AgentList,
    Call::SendKeys {
      target: "k".into(),
      keys: vec!["esc".into()],
    },
    Call::Wait {
      target: "k".into(),
      until: vec!["idle".into()],
      timeout_ms: 10_000,
    },
    Call::PromptWait {
      target: "k".into(),
      text: "go".into(),
      until: vec!["working".into(), "blocked".into()],
      timeout_ms: 60_000,
    },
  ]);
  calls
}

#[test]
fn every_call_sends_only_params_the_pinned_herdr_schema_knows() {
  let schema: Value = serde_json::from_str(SCHEMA).expect("schema");
  assert_eq!(schema["protocol"], json!(crate::herdr::HERDR_PROTOCOL));
  for call in every_call() {
    let entry = &schema["methods"][call.method()];
    let known: BTreeSet<&str> = entry["params"]
      .as_array()
      .expect("params")
      .iter()
      .filter_map(Value::as_str)
      .collect();
    let params = call.params();
    let sent: BTreeSet<&str> = params
      .as_object()
      .expect("object")
      .keys()
      .map(String::as_str)
      .collect();
    assert!(
      sent.is_subset(&known),
      "{}: {sent:?} not in {known:?}",
      call.method()
    );
    for required in entry["required"]
      .as_array()
      .expect("required")
      .iter()
      .filter_map(Value::as_str)
    {
      assert!(
        sent.contains(required),
        "{} misses {required}",
        call.method()
      );
    }
  }
}

#[test]
fn cli_forms_name_their_herdr_subcommands() {
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let lines: Vec<String> = every_call()
    .iter()
    .map(|call| call.argv(&env).join(" "))
    .collect();
  assert!(lines.contains(&"herdr workspace list".to_owned()));
  assert!(
    lines.contains(&"herdr worktree open --cwd /w --path /w/t --label k --no-focus".to_owned())
  );
  assert!(lines.contains(&"herdr pane process-info --pane w1:p1".to_owned()));
  assert!(lines.contains(&"herdr agent send-keys k esc".to_owned()));
  assert!(lines.contains(&"herdr agent wait k --until idle --timeout 10000".to_owned()));
  assert!(lines.contains(
    &"herdr agent prompt k go --wait --until working --until blocked --timeout 60000".to_owned()
  ));
}

#[test]
fn cli_deadlines_cover_the_call_s_own_wait() {
  let start = &crate::herdr_argv::sample_calls()[0];
  assert_eq!(start.cli_timeout(), Duration::from_secs(31));
  assert_eq!(Call::AgentList.cli_timeout(), Duration::from_secs(30));
  let wait = Call::Wait {
    target: "k".into(),
    until: Vec::new(),
    timeout_ms: 90_000,
  };
  assert_eq!(wait.cli_timeout(), Duration::from_secs(120));
  assert_eq!(Call::WorkspaceList.cli_timeout(), Duration::from_secs(120));
  let open = Call::WorktreeOpen {
    cwd: "/w".into(),
    path: "/w/t".into(),
    label: "k".into(),
  };
  assert_eq!(open.cli_timeout(), Duration::from_secs(120));
  let create = crate::herdr_argv::sample_calls()
    .into_iter()
    .find(|call| matches!(call, Call::Create { .. }));
  assert_eq!(
    create.map(|call| call.cli_timeout()),
    Some(Duration::from_secs(120))
  );
}

#[test]
fn replies_give_their_result_or_a_typed_error() {
  assert_eq!(
    result_of("agent.list", r#"{"id":"1","result":{"agents":[]}}"#),
    Ok(json!({"agents": []}))
  );
  assert_eq!(
    result_of("agent.list", r#"{"agents":[]}"#),
    Ok(json!({"agents": []}))
  );
  let stalled = result_of(
    "agent.prompt",
    r#"{"error":{"code":"agent_prompt_stalled","message":"no echo"}}"#,
  );
  assert_eq!(
    stalled,
    Err(CallError {
      message: "herdr agent.prompt: agent_prompt_stalled: no echo".into(),
      code: Some("agent_prompt_stalled".into()),
      exit_code: None,
      uncertain: true,
    })
  );
  let refused = result_of(
    "worktree.create",
    r#"{"error":{"code":"worktree_create_failed","message":"exists"}}"#,
  )
  .expect_err("error");
  assert!(!refused.uncertain);
  assert_eq!(
    result_of("x", "garbage").expect_err("json").message,
    "herdr x: non-JSON output: garbage"
  );
  assert_eq!(
    result_of("x", "[1]").expect_err("object").message,
    "herdr x: expected object JSON"
  );
}

#[test]
fn a_missing_herdr_is_a_certain_failure() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([
    ("HOME", tmp.path().display().to_string()),
    (
      "HERDR_SOCKET_PATH",
      tmp.path().join("absent.sock").display().to_string(),
    ),
  ]);
  let err = request(
    &mut Client::new(),
    &Paths::at(tmp.path()),
    &env,
    &Call::AgentList,
  )
  .expect_err("absent");
  assert!(
    err.message.starts_with("herdr agent.list:"),
    "{}",
    err.message
  );
  assert!(!err.uncertain);
}

#[test]
fn socket_error_objects_and_non_json_replies_are_typed_failures() {
  use crate::herdr_state::Scripted;
  let tmp = tempfile::tempdir().expect("temp");
  let herdr = crate::herdr_fixture::HerdrFixture::start();
  let error = |code: &str| Scripted::Error {
    code: code.to_owned(),
    message: "scripted".to_owned(),
  };
  herdr.script("agent.prompt", error("timeout"));
  herdr.script("agent.send_keys", error("agent_not_found"));
  herdr.script("agent.list", Scripted::Raw("garbage".to_owned()));
  let paths = Paths::at(tmp.path());
  let env = herdr.env();
  let mut client = Client::new();
  let prompt = Call::Prompt {
    target: "k".into(),
    text: "go".into(),
  };
  let timed = request(&mut client, &paths, &env, &prompt).expect_err("timeout");
  assert_eq!(
    (timed.code.as_deref(), timed.uncertain),
    (Some("timeout"), true)
  );
  let keys = Call::SendKeys {
    target: "k".into(),
    keys: vec!["esc".into()],
  };
  let missing = request(&mut client, &paths, &env, &keys).expect_err("missing");
  assert_eq!(
    (missing.code.as_deref(), missing.uncertain),
    (Some("agent_not_found"), false)
  );
  let raw = request(&mut client, &paths, &env, &Call::AgentList).expect_err("raw");
  assert_eq!(raw.message, "herdr agent.list: non-JSON output: garbage");
  assert_eq!(
    request(&mut client, &paths, &env, &Call::AgentList),
    Ok(json!({"agents": []}))
  );
}

#[test]
fn a_cli_read_answers_its_plain_text_and_a_failed_read_fails() {
  use std::os::unix::fs::PermissionsExt;
  use std::os::unix::net::UnixListener;
  let tmp = tempfile::tempdir().expect("temp");
  let bin = tmp.path().join("bin");
  std::fs::create_dir_all(&bin).expect("bin");
  let script = bin.join("herdr");
  let body = "#!/bin/sh\n[ \"$3\" = gone ] && { echo 'agent not found' >&2; exit 1; }\n\
    printf 'line one\\nusage limit reached\\n'\n";
  std::fs::write(&script, body).expect("script");
  std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
  let sock = tmp.path().join("herdr.sock");
  let listener = UnixListener::bind(&sock).expect("bind");
  let peer = std::thread::spawn(move || {
    let (mut stream, _) = listener.accept().expect("accept");
    let ping = crate::client::read_json(&mut stream).expect("ping");
    let reply = json!({"id": ping.get("id"), "result": {"protocol": 99}});
    crate::client::write_json(&mut stream, &reply).expect("pong");
  });
  let env = Env::from_pairs([
    ("PATH", format!("{}:/usr/bin:/bin", bin.display())),
    ("HOME", tmp.path().display().to_string()),
    ("HERDR_SOCKET_PATH", sock.display().to_string()),
  ]);
  let paths = Paths::at(tmp.path());
  let mut client = Client::new();
  let read = |target: &str| Call::Read {
    target: target.to_owned(),
  };
  assert_eq!(
    request(&mut client, &paths, &env, &read("k")),
    Ok(json!({"read": {"text": "line one\nusage limit reached\n"}}))
  );
  peer.join().expect("peer");
  let err = request(&mut client, &paths, &env, &read("gone")).expect_err("gone");
  assert_eq!(err.message, "herdr agent.read: failed (1): agent not found");
  assert_eq!(err.exit_code, Some(1));
  assert_eq!(crate::launch::Failure::from(err).exit_code, Some(1));
}

#[test]
fn a_prompt_is_delivered_by_plain_text_or_a_reply_without_an_error() {
  use super::delivered;
  assert_eq!(delivered("agent.prompt", "prompted k\n"), Ok(()));
  assert_eq!(
    delivered("agent.prompt", r#"{"result":{"ok":true}}"#),
    Ok(())
  );
  assert_eq!(
    delivered(
      "agent.prompt",
      r#"{"error":{"code":"agent_not_found","message":"no k"}}"#
    ),
    Err("herdr agent.prompt: agent_not_found: no k".to_owned())
  );
}
