use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;

use super::{Sandbox, assert_silent, called, context_of, mandate};

#[test]
fn the_sandbox_plugin_holds_the_skill_and_no_shim() {
  let sb = Sandbox::new();
  assert!(sb.plugin.join("skills/jev/SKILL.md").is_file());
  assert!(!sb.plugin.join("scripts").exists());
}

#[test]
fn the_called_command_is_read_from_a_mandate() {
  assert_eq!(called(&mandate("skill")), "toolu jev");
}

#[test]
fn silence_and_context_are_told_apart() {
  assert_silent(&Outcome {
    exit: Exit::Success,
    stdout: None,
    stderr: None,
  });
  let text = r#"{"hookSpecificOutput":{"hookEventName":"E","additionalContext":"c"}}"#;
  assert_eq!(context_of(&Outcome::data(text.to_owned()), "E"), "c");
  let caught = std::panic::catch_unwind(|| context_of(&Outcome::data(format!("{text}\n")), "E"));
  assert!(caught.is_err());
}
