use super::{BREW_INSTALL, INSTALLER, missing_message};

/// Characters a cmd `echo`, sh single quotes or a JSON string would reinterpret.
const UNSAFE: &[char] = &['(', ')', '&', '<', '>', '^', '%', '"', '\'', '\\'];

#[test]
fn the_missing_message_names_the_plugin_and_both_install_commands() {
  let message = missing_message("jev");
  assert!(message.starts_with("jev plugin: no usable native toolu - checked TOOLU_BIN"));
  assert!(message.contains(INSTALLER));
  assert!(message.contains(BREW_INSTALL));
  assert!(message.contains("~/.local/bin and PATH"));
  assert!(message.contains("If TOOLU_BIN is set, it must point to a native toolu"));
}

#[test]
fn messages_are_one_cmd_safe_line_whose_only_pipe_is_the_installer() {
  for message in [missing_message("toolu"), missing_message("jev")] {
    assert!(!message.contains('\n'), "{message}");
    assert!(!message.contains(UNSAFE), "{message}");
    assert!(!message.contains(" hook "), "{message}");
    assert_eq!(message.matches('|').count(), 1, "{message}");
  }
}
