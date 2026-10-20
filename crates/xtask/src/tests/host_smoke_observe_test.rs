use super::observe;

const FAILING: &str = "{\"status\": \"failing\"}";

#[test]
fn observe_waits_for_a_failing_gate_file() {
  let dir = tempfile::tempdir().unwrap();
  let (gate, source) = (dir.path().join("gate.json"), dir.path().join("sample.ts"));
  std::fs::write(&source, "edited\n").unwrap();
  assert!(observe(&gate, &source).is_none());
  std::fs::write(&gate, "{\"status\": \"passing\"}").unwrap();
  assert!(observe(&gate, &source).is_none());
  std::fs::write(&gate, FAILING).unwrap();
  assert_eq!(
    observe(&gate, &source),
    Some((FAILING.to_owned(), "edited\n".to_owned()))
  );
}
