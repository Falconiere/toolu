use super::normalize_command;

#[test]
fn a_launcher_command_is_the_bundle_path() {
  let command = "exec \"${CLAUDE_PLUGIN_ROOT}/hooks/dist/pre-tools.js\"";
  assert_eq!(normalize_command(command), "hooks/dist/pre-tools.js");
}
