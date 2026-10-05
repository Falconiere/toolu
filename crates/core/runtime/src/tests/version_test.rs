use super::{Comparison, compare};

#[test]
fn identical_strings_are_the_same() {
  assert_eq!(compare("7.10.0", "7.10.0"), Comparison::Same);
  assert_eq!(compare("garbage", "garbage"), Comparison::Same);
}

#[test]
fn triples_compare_numerically_major_included() {
  assert_eq!(compare("7.10.0", "7.11.0"), Comparison::Older);
  assert_eq!(compare("7.10.0", "7.9.0"), Comparison::Newer);
  assert_eq!(compare("7.10.0", "8.0.0"), Comparison::Older);
  assert_eq!(compare("10.0.0", "9.99.99"), Comparison::Newer);
}

#[test]
fn suffixes_are_ignored_but_still_differ() {
  assert_eq!(compare("7.10.0", "7.10.0-rc.1"), Comparison::Differs);
  assert_eq!(compare("7.10.0+b", "7.10.0"), Comparison::Differs);
  assert_eq!(compare("7.10.0-rc.1", "7.11.0"), Comparison::Older);
}

#[test]
fn anything_else_is_incomparable() {
  for plugin in [
    "", "7.10", "7.10.0.1", "v7.10.0", "7.x.0", "7..0", "garbage",
  ] {
    assert_eq!(
      compare("7.10.0", plugin),
      Comparison::Incomparable,
      "{plugin}"
    );
  }
}
