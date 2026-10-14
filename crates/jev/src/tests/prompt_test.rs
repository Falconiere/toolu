use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::user_prompt_submit;
use crate::hooks::{KEY, Sandbox, assert_silent, called, context_of, skill_file};
use crate::session::session_start;

const EVENT: &str = "UserPromptSubmit";

fn prompt(sb: &Sandbox, env: &Env, text: &str) -> Outcome {
  let payload = format!("{{\"prompt\":{}}}", serde_json::Value::from(text));
  user_prompt_submit(env, &sb.plugin, Some(&payload))
}

fn published(host: &str) -> (Sandbox, Env) {
  let sb = Sandbox::new();
  let env = sb.env(host);
  context_of(&session_start(&env, &sb.plugin), "SessionStart");
  (sb, env)
}

#[test]
fn names_toolu_jev_for_a_task_and_stays_silent_for_confirmations() {
  let (sb, env) = published("claude");
  let context = context_of(&prompt(&sb, &env, "rank these approaches"), EVENT);
  assert_eq!(called(&context), "toolu jev");
  assert!(
    context.starts_with("Jev is mandatory for this task when it contains semantic decisions.")
  );
  assert!(context.contains("The command does not load a project .env file."));
  assert!(context.contains(&format!(
    "Syntax and linked examples: {}.",
    skill_file(&sb.plugin)
  )));
  assert!(!context.contains(KEY) && !context.to_lowercase().contains("bun"));
  assert_silent(&prompt(&sb, &env, "LGTM"));
}

#[test]
fn codex_path_slash_command_multiline_task_and_missing_key() {
  let sb = Sandbox::new();
  let codex = sb.dir.path().join("codex profile");
  let env = sb.env("codex").with("CODEX_HOME", codex.to_str().unwrap());
  context_of(&session_start(&env, &sb.plugin), "SessionStart");
  assert!(!codex.join("jev/jev.sh").exists(), "nothing is published");
  let slash = prompt(&sb, &env, "/delivery-flow:delivery-flow ship export");
  assert_eq!(called(&context_of(&slash, EVENT)), "toolu jev");
  let multiline = prompt(&sb, &env, "ok\nnow add pagination");
  context_of(&multiline, EVENT);
  assert_silent(&prompt(&sb, &env, "\n\ty\t\n"));
  let bare = Env::from_pairs([
    ("HOME", sb.home.to_str().unwrap()),
    ("TOOLU_HOST_OVERRIDE", "codex"),
    ("CODEX_HOME", codex.to_str().unwrap()),
    ("PATH", "/nonexistent"),
  ]);
  let context = context_of(&prompt(&sb, &bare, "rank these"), EVENT);
  assert!(context.starts_with("The Jev hook did not receive TYPESAFE_API_KEY."));
  assert!(context.contains("command environment without printing its value"));
  assert!(context.contains("you MUST call toolu jev before"));
  assert!(!context.contains("Jev unavailable (missing:"));
}

#[test]
fn trivial_prompts_print_nothing_and_near_misses_do_print() {
  let (sb, env) = published("claude");
  let trivial = [
    "y",
    "N",
    "Yes.",
    "ok!",
    "thank you",
    "Go Ahead",
    "LGTM?",
    "  continue  ",
    "\tdone\n",
  ];
  for text in trivial {
    assert_silent(&prompt(&sb, &env, text));
  }
  for text in [
    "yes please",
    "yes!!",
    "yes .",
    "ok then",
    "thanks a lot",
    ".",
  ] {
    context_of(&prompt(&sb, &env, text), EVENT);
  }
}

#[test]
fn invalid_prompt_input_is_silent_and_no_wrapper_is_needed() {
  let sb = Sandbox::new();
  let env = sb.env("claude");
  for stdin in [
    None,
    Some(""),
    Some("bad json"),
    Some("{}"),
    Some("[]"),
    Some("null"),
    Some(r#"{"prompt":3}"#),
    Some(r#"{"prompt":""}"#),
    Some(r#"{"text":"rank these"}"#),
  ] {
    assert_silent(&user_prompt_submit(&env, &sb.plugin, stdin));
  }
  let context = context_of(&prompt(&sb, &env, "rank these"), EVENT);
  assert_eq!(called(&context), "toolu jev");
}
