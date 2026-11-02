use super::{credential, safe};

#[test]
fn identifiers_and_credentials_reject_unsafe_or_unbounded_values() {
  for bad in ["", ".", "..", "../escape", "a/b", "é", &"a".repeat(129)] {
    assert!(!safe(bad), "{bad}");
  }
  assert!(safe(&"a".repeat(128)));
  assert!(!credential(&"a".repeat(31)));
  assert!(!credential(&"a".repeat(257)));
  assert!(!credential(&format!("{}\n", "a".repeat(32))));
  assert!(credential(&"a".repeat(32)));
}
