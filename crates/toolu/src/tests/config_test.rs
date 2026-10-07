use clap::Command;

use super::command;

#[test]
fn config_offers_get_set_and_validate() {
  let config = command();
  let names: Vec<_> = config.get_subcommands().map(Command::get_name).collect();
  assert_eq!(names, ["get", "set", "validate"]);
}
