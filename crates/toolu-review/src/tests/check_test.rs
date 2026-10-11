use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::check_binary;

#[test]
fn a_shell_without_toolu_gets_one_context_line_without_a_trailing_newline() {
  let env = Env::from_pairs([("PATH", "/nonexistent")]);
  let outcome = check_binary(&env, None);
  assert_eq!(outcome.exit, Exit::Success);
  let text = outcome.stdout.unwrap();
  assert!(text.contains("toolu: native binary"), "{text}");
  assert!(!text.ends_with('\n'));
}
