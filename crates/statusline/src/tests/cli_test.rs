use super::command;

#[test]
fn force_belongs_to_setup_only() {
  assert!(
    command()
      .try_get_matches_from(["statusline", "setup", "--force"])
      .is_ok()
  );
  assert!(
    command()
      .try_get_matches_from(["statusline", "render", "--force"])
      .is_err()
  );
}
