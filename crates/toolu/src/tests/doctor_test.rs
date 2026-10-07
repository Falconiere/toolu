use super::command;

#[test]
fn doctor_is_a_command_without_a_planned_verb() {
  assert!(command().try_get_matches_from(["doctor"]).is_ok());
  assert!(
    command()
      .try_get_matches_from(["doctor", "planned"])
      .is_err()
  );
}
