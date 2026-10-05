use super::{BREW_INSTALL, INSTALLER, fallback_advisory, missing_message};

/// Characters a cmd `echo`, sh single quotes or a JSON string would reinterpret.
const UNSAFE: &[char] = &['(', ')', '&', '<', '>', '^', '%', '"', '\'', '\\'];

#[test]
fn the_missing_message_names_the_plugin_and_both_install_commands() {
  let message = missing_message("jev");
  assert!(message.starts_with("jev plugin: toolu is not installed - checked TOOLU_BIN"));
  assert!(message.contains(INSTALLER));
  assert!(message.contains(BREW_INSTALL));
  assert!(message.contains("~/.local/bin and PATH"));
}

#[test]
fn the_fallback_advisory_names_the_transition_and_both_install_commands() {
  let message = fallback_advisory("toolu");
  assert!(message.starts_with("toolu plugin: native toolu not found, running the Bun bundle"));
  assert!(message.contains("#425"));
  assert!(message.contains(INSTALLER) && message.contains(BREW_INSTALL));
}

#[test]
fn messages_are_one_cmd_safe_line_whose_only_pipe_is_the_installer() {
  for message in [missing_message("toolu"), fallback_advisory("toolu")] {
    assert!(!message.contains('\n'), "{message}");
    assert!(!message.contains(UNSAFE), "{message}");
    assert!(!message.contains(" hook "), "{message}");
    assert_eq!(message.matches('|').count(), 1, "{message}");
  }
}
