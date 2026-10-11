use super::glob_pattern;

#[test]
fn a_single_star_does_not_cross_a_slash() {
  let pattern = glob_pattern("plugins/*/hooks/hooks.json");
  let regex = regex::Regex::new(&pattern).expect("pattern");
  assert!(regex.is_match("plugins/toolu/hooks/hooks.json"));
  assert!(!regex.is_match("plugins/toolu/hooks/src/hooks.json"));
}
