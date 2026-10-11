use std::path::Path;

use super::render;

#[test]
fn status_api_doc_matches_schema_definitions() {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/epic-status-api.md");
  let generated = render();
  if std::env::var_os("TOOLU_UPDATE_DOCS").is_some() {
    std::fs::write(&path, generated).expect("write generated API reference");
    return;
  }
  let committed = std::fs::read_to_string(&path).expect("generated API reference");
  assert_eq!(committed, generated, "regenerate epic status API reference");
}
