use super::spec_of;

#[test]
fn the_spec_runs_to_the_first_double_underscore() {
  assert_eq!(spec_of("x@t__a.sh"), Some("x@t"));
  assert_eq!(spec_of("x@t__a__b.js"), Some("x@t"));
  assert_eq!(spec_of("x y@t__m.json"), Some("x y@t"));
  for name in [".x@t__a.sh", "__a.sh", "noname.sh", "x@t__a.txt", "x@t__a"] {
    assert_eq!(spec_of(name), None, "{name}");
  }
}
