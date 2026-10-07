use super::epic_key;

#[test]
fn a_directory_name_is_the_epic_key_without_a_graph() {
  let key = epic_key(std::path::Path::new("/epics/falconiere-toolu-402"));
  assert_eq!(key, "falconiere-toolu-402");
}
