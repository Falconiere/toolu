#[test]
fn embedded_profiles_are_valid_templates() {
  for profile in &super::super::PROFILES {
    assert!(super::valid(profile.body, profile), "{}", profile.name);
  }
}
