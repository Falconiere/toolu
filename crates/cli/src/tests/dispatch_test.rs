use std::path::PathBuf;

use serde_json::{Value, json};
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::{Ctx, Outcome};

use super::{ctx_of, finish, run, wants_json};
use crate::tests::{context, words};
use crate::tree;

fn dispatched(line: &str) -> Outcome {
  run(&words(line), &context(), &tree::command)
}

fn document(outcome: &Outcome) -> Value {
  serde_json::from_str(outcome.stdout.as_deref().unwrap()).unwrap()
}

#[test]
fn a_typo_is_a_usage_error_that_suggests_the_namespace() {
  let outcome = dispatched("epik start");
  assert_eq!(outcome.exit, Exit::Usage);
  assert_eq!(outcome.stdout, None);
  let stderr = outcome.stderr.unwrap();
  assert!(
    stderr.contains("unrecognized subcommand 'epik'"),
    "{stderr}"
  );
  assert!(
    stderr.contains("a similar subcommand exists: 'epic'"),
    "{stderr}"
  );
}

#[test]
fn under_json_a_usage_error_is_also_one_document() {
  let outcome = dispatched("--json epik start");
  assert_eq!(outcome.exit, Exit::Usage);
  assert_eq!(
    document(&outcome),
    json!({ "error": {
      "code": 64,
      "name": "usage",
      "message": "unrecognized subcommand 'epik'",
      "suggestion": "epic",
    }})
  );
  assert!(outcome.stderr.is_some());
}

#[test]
fn json_status_is_one_document() {
  let outcome = dispatched("epic status 402 --json");
  assert_eq!(outcome.exit, Exit::Success);
  assert!(document(&outcome).get("engine").is_some());
  assert!(document(&outcome).get("issues").is_some());
}

#[test]
fn a_missing_verb_prints_help_on_stderr() {
  let outcome = dispatched("epic");
  assert_eq!(outcome.exit, Exit::Usage);
  assert_eq!(outcome.stdout, None);
  assert!(outcome.stderr.unwrap().contains("Usage: toolu epic"));
}

#[test]
fn flags_without_a_command_are_a_usage_error() {
  let outcome = dispatched("--json");
  assert_eq!(outcome.exit, Exit::Usage);
  assert_eq!(
    document(&outcome)["error"]["message"],
    "toolu: a command is required"
  );
  assert!(outcome.stderr.unwrap().contains("Usage: toolu"));
}

#[test]
fn the_hook_protocol_flag_works_beside_other_flags() {
  assert_eq!(
    dispatched("--hook-protocol --quiet"),
    Outcome::data("1".to_owned())
  );
}

#[test]
fn a_namespace_verb_runs_with_global_flags_before_or_after_it() {
  let text = dispatched("epic planned");
  assert!(
    text
      .stdout
      .unwrap()
      .starts_with("toolu epic is not ported yet")
  );
  for line in ["--json pr-babysit planned", "pr-babysit planned --json"] {
    let outcome = dispatched(line);
    assert_eq!(document(&outcome)["namespace"], "babysit", "{line}");
  }
}

#[test]
fn a_plugin_name_alias_reaches_its_namespace() {
  let outcome = dispatched("--json pr-babysit planned");
  assert_eq!(document(&outcome)["namespace"], "babysit");
}

#[test]
fn hooks_through_clap_keep_their_own_protocol() {
  let blocked = dispatched("--json hook no-such-hook --event PreToolUse");
  assert_eq!(blocked.exit, Exit::Blocked);
  assert_eq!(blocked.stdout, None);
  let context = dispatched("--quiet jev hook session-start --event SessionStart");
  assert_eq!(context.exit, Exit::Success);
  let message: Value = serde_json::from_str(&context.stdout.unwrap()).unwrap();
  let message = message["systemMessage"].as_str().unwrap().to_owned();
  assert!(message.starts_with("jev plugin: toolu "), "{message}");
}

#[test]
fn an_unknown_host_is_a_usage_error() {
  let outcome = dispatched("--host bogus commands");
  assert_eq!(outcome.exit, Exit::Usage);
  assert!(outcome.stderr.unwrap().contains("possible values"));
}

#[test]
fn the_global_flags_become_the_verb_context() {
  let argv = "toolu --host codex --config-dir /tmp/cfg -q epic planned --json";
  let matches = tree::command()
    .try_get_matches_from(argv.split(' '))
    .unwrap();
  assert_eq!(
    ctx_of(&matches),
    Ctx {
      json: true,
      quiet: true,
      host: Some(Host::Codex),
      config_dir: Some(PathBuf::from("/tmp/cfg")),
    }
  );
}

#[test]
fn a_tree_command_missing_from_the_registry_is_an_internal_failure() {
  let tree = || tree::command().subcommand(clap::Command::new("ghost"));
  let outcome = run(&words("ghost"), &context(), &tree);
  assert_eq!(
    outcome,
    Outcome::failed(Exit::Failure, "toolu: no namespace ghost".to_owned())
  );
}

#[test]
fn json_is_noticed_only_before_the_separator() {
  assert!(wants_json(&words("epik --json")));
  assert!(!wants_json(&words("epik -- --json")));
}

#[test]
fn finish_fills_json_and_honours_quiet() {
  let json = Ctx {
    json: true,
    quiet: true,
    ..Ctx::default()
  };
  let quiet_success = Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: Some("advice".to_owned()),
  };
  assert_eq!(
    finish(quiet_success, &json),
    Outcome {
      exit: Exit::Success,
      stdout: Some("{}".to_owned()),
      stderr: None,
    }
  );
  let failed = finish(
    Outcome::failed(Exit::Unavailable, "gh is missing\nmore".to_owned()),
    &json,
  );
  assert_eq!(failed.stderr.as_deref(), Some("gh is missing\nmore"));
  let doc: Value = serde_json::from_str(&failed.stdout.unwrap()).unwrap();
  assert_eq!(
    doc,
    json!({ "error": { "code": 69, "name": "unavailable", "message": "gh is missing", "suggestion": null } })
  );
}

#[test]
fn a_silent_failure_s_json_message_is_the_exit_meaning() {
  let json = Ctx {
    json: true,
    ..Ctx::default()
  };
  for stderr in [None, Some(String::new())] {
    let silent = Outcome {
      exit: Exit::Failure,
      stdout: None,
      stderr,
    };
    let doc: Value = serde_json::from_str(&finish(silent, &json).stdout.unwrap()).unwrap();
    assert_eq!(doc["error"]["message"], "the command ran and failed");
  }
}
