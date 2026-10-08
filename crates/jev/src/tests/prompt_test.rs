use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::user_prompt_submit;
use crate::hooks::{KEY, Sandbox, assert_silent, called, context_of, skill_file};
use crate::session::session_start;

const EVENT: &str = "UserPromptSubmit";
const OPENCODE_SKILL: &str = "Syntax and linked examples: skill({ name: \"jev-jev\" }).";

fn prompt(sb: &Sandbox, env: &Env, text: &str) -> Outcome {
  let payload = format!("{{\"prompt\":{}}}", serde_json::Value::from(text));
  user_prompt_submit(env, &sb.plugin, Some(&payload))
}

fn published(host: &str) -> (Sandbox, Env) {
  let sb = Sandbox::new();
  let env = sb.env(host);
  context_of(&session_start(&env, &sb.plugin, Some("{}")), "SessionStart");
  (sb, env)
}

fn opencode(sb: &Sandbox) -> Env {
  let state = sb.dir.path().join("project/.opencode/toolu/state");
  let env = sb
    .env("opencode")
    .with("TOOLU_CONFIG_DIR", state.to_str().unwrap());
  env.with(
    "TOOLU_PROJECT_DIR",
    sb.dir.path().join("project").to_str().unwrap(),
  )
}

#[test]
fn names_toolu_jev_for_a_task_and_stays_silent_for_confirmations() {
  let (sb, env) = published("claude");
  let context = context_of(&prompt(&sb, &env, "rank these approaches"), EVENT);
  assert_eq!(called(&context), "toolu jev");
  assert!(
    context.starts_with("Jev is mandatory for this task when it contains semantic decisions.")
  );
  assert!(context.contains("The command is toolu jev and does not load a project .env file."));
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
  context_of(&session_start(&env, &sb.plugin, Some("{}")), "SessionStart");
  assert!(codex.join("jev/jev.sh").is_symlink());
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
fn an_unpublished_wrapper_and_invalid_prompt_input_are_silent() {
  let sb = Sandbox::new();
  let env = sb.env("claude");
  for stdin in [
    None,
    Some(""),
    Some("bad json"),
    Some("{}"),
    Some(r#"{"prompt":"rank these"}"#),
  ] {
    assert_silent(&user_prompt_submit(&env, &sb.plugin, stdin));
  }
  let (sb, env) = published("claude");
  for stdin in [
    "[]",
    "null",
    r#"{"prompt":3}"#,
    r#"{"prompt":""}"#,
    r#"{"text":"rank these"}"#,
  ] {
    assert_silent(&user_prompt_submit(&env, &sb.plugin, Some(stdin)));
  }
}

#[test]
fn a_users_override_is_quoted_and_a_non_executable_one_is_silent() {
  let (sb, env) = published("claude");
  let dst = sb.claude_dir().join("jev.sh");
  fs::remove_file(&dst).unwrap();
  fs::write(&dst, "#!/bin/sh\n").unwrap();
  assert_silent(&prompt(&sb, &env, "rank these"));
  fs::set_permissions(&dst, fs::Permissions::from_mode(0o755)).unwrap();
  let context = context_of(&prompt(&sb, &env, "rank these"), EVENT);
  assert_eq!(called(&context), format!("'{}'", dst.display()));
}

#[test]
fn opencode_startup_names_the_native_skill_and_toolu_jev() {
  let sb = Sandbox::new();
  let env = opencode(&sb);
  let payload = r#"{"hook_event_name":"SessionStart","source":"startup"}"#;
  let context = context_of(
    &session_start(&env, &sb.plugin, Some(payload)),
    "SessionStart",
  );
  let wrapper = sb
    .dir
    .path()
    .join("project/.opencode/toolu/state/jev/jev.sh");
  assert_eq!(
    fs::read_link(wrapper).unwrap(),
    sb.plugin.join("scripts/jev.sh")
  );
  assert_eq!(called(&context), "toolu jev");
  assert!(context.contains(OPENCODE_SKILL));
  assert!(!context.contains("skills/jev/SKILL.md") && !context.contains(KEY));
  assert!(!context.to_lowercase().contains("bun"));
}

#[test]
fn opencode_compaction_relinks_the_wrapper_and_adds_no_second_mandate() {
  let sb = Sandbox::new();
  let env = opencode(&sb);
  context_of(
    &session_start(&env, &sb.plugin, Some(r#"{"source":"startup"}"#)),
    "SessionStart",
  );
  let wrapper = sb
    .dir
    .path()
    .join("project/.opencode/toolu/state/jev/jev.sh");
  fs::remove_file(&wrapper).unwrap();
  let compact = r#"{"hook_event_name":"SessionStart","source":"compact"}"#;
  assert_silent(&session_start(&env, &sb.plugin, Some(compact)));
  assert_eq!(
    fs::read_link(wrapper).unwrap(),
    sb.plugin.join("scripts/jev.sh")
  );
}

#[test]
fn opencode_prompt_reminder_carries_the_same_command_and_skill() {
  let sb = Sandbox::new();
  let env = opencode(&sb);
  context_of(
    &session_start(&env, &sb.plugin, Some(r#"{"source":"startup"}"#)),
    "SessionStart",
  );
  let context = context_of(&prompt(&sb, &env, "rank these two designs"), EVENT);
  assert_eq!(called(&context), "toolu jev");
  assert!(context.contains(OPENCODE_SKILL) && !context.contains(KEY));
  assert_silent(&prompt(&sb, &env, "ok"));
}

#[test]
fn opencode_keeps_a_users_override_and_claude_restates_after_compaction() {
  let sb = Sandbox::new();
  let env = opencode(&sb);
  let dst = sb
    .dir
    .path()
    .join("project/.opencode/toolu/state/jev/jev.sh");
  fs::create_dir_all(dst.parent().unwrap()).unwrap();
  fs::write(&dst, "#!/bin/sh\necho user-override\n").unwrap();
  fs::set_permissions(&dst, fs::Permissions::from_mode(0o755)).unwrap();
  let startup = session_start(&env, &sb.plugin, Some(r#"{"source":"startup"}"#));
  let context = context_of(&startup, "SessionStart");
  assert_eq!(called(&context), format!("'{}'", dst.display()));
  let claude = Sandbox::new();
  let compact = session_start(
    &claude.env("claude"),
    &claude.plugin,
    Some(r#"{"source":"compact"}"#),
  );
  let context = context_of(&compact, "SessionStart");
  assert!(context.contains(&skill_file(&claude.plugin)) && !context.contains("--no-env-file"));
  assert!(claude.claude_dir().join("jev.sh").exists());
}
