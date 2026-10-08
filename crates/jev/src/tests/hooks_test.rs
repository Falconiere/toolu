use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;

use super::{Sandbox, assert_silent, called, context_of, mandate, sh};

#[test]
fn the_sandbox_shim_is_the_one_the_plugin_will_ship() {
  let sb = Sandbox::new();
  let shim = sb.plugin.join("scripts/jev.sh");
  assert_eq!(
    fs::read_to_string(&shim).unwrap(),
    "#!/bin/sh\nexec toolu jev \"$@\"\n"
  );
  assert_eq!(
    fs::metadata(&shim).unwrap().permissions().mode() & 0o777,
    0o755
  );
  assert!(sb.plugin.join("skills/jev/SKILL.md").is_file());
}

#[test]
fn the_called_command_is_read_from_a_mandate() {
  assert_eq!(called(&mandate("toolu jev", "skill")), "toolu jev");
  assert_eq!(
    sh("echo \"$X\"", &Env::from_pairs([("X", "1")])).stdout,
    "1\n"
  );
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
