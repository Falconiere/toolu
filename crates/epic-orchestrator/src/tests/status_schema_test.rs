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

#[test]
fn status_api_doc_documents_the_private_network_fleet() {
  let doc = render();
  for phrase in [
    "| `/api/v1/fleet` |",
    "| `/api/v1/fleet/peers/{name}/{route}` |",
    "## Fleet over a private network",
    "the URL names the peer's bound IP and port",
    "header_up Host {upstream_hostport}",
    "must not be exposed to the public internet without its token and TLS in front of it",
    "TOOLU_EPIC_PEER_TOKENS",
    "`version-mismatch`",
  ] {
    assert!(doc.contains(phrase), "missing {phrase}");
  }
}
