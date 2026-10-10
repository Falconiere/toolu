use super::{program_at, program_indexes};

#[test]
fn finds_real_shell_launchers() {
  let parsed = toolu_shell::analyze("find src -exec rg foo {} +");
  let command = parsed.commands.first().expect("find command");
  let positions = program_indexes(command);
  assert_eq!(positions.len(), 2);
  assert_eq!(
    positions.get(1).and_then(|at| program_at(command, *at)),
    Some("rg")
  );
}
