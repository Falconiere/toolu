use std::path::Path;

use super::{PLUGIN, command};

#[test]
fn native_command_exposes_public_and_internal_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  for verb in ["render", "setup", "refresh"] {
    assert!(
      command().try_get_matches_from(["statusline", verb]).is_ok(),
      "{verb}"
    );
  }
  assert!(
    command()
      .try_get_matches_from(["statusline", "refresh-cache", "/repo"])
      .is_ok()
  );
  assert!(
    command()
      .try_get_matches_from(["statusline", "planned"])
      .is_err()
  );
}
