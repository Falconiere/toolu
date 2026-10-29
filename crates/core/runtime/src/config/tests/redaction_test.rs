use toolu_protocol::host::Host;

use super::{redact_window, redaction_fingerprint, redaction_overlap};
use crate::config::secrets::load;
use crate::env::Env;
use crate::host::roots::Roots;

#[test]
fn a_secret_crossing_either_page_edge_has_no_visible_fragment() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().display().to_string();
  let env = Env::from_pairs([
    ("HOME", home.clone()),
    ("TOOLU_CONFIG_DIR", home),
    ("TOOLU_EPIC_STATUS_TOKEN", "SECRET-LOG-CANARY".to_owned()),
  ]);
  let secrets = load(&Roots::new(env, Some(Host::Codex))).unwrap();
  let full = b"leftSECRET-LOG-CANARYright";
  assert_eq!(redaction_overlap(&secrets), 16);
  assert_eq!(
    redact_window(full, 6, full.len(), &secrets),
    "<redacted>right"
  );
  assert_eq!(redact_window(full, 0, 6, &secrets), "left<redacted>");
}

#[test]
fn redaction_fingerprint_changes_with_resolved_secret_values() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().display().to_string();
  let secrets = |token: &str| {
    let env = Env::from_pairs([
      ("HOME", home.clone()),
      ("TOOLU_CONFIG_DIR", home.clone()),
      ("TOOLU_EPIC_STATUS_TOKEN", token.to_owned()),
    ]);
    load(&Roots::new(env, Some(Host::Codex))).unwrap()
  };
  assert_eq!(
    redaction_fingerprint(&secrets("first")),
    redaction_fingerprint(&secrets("first"))
  );
  assert_ne!(
    redaction_fingerprint(&secrets("first")),
    redaction_fingerprint(&secrets("rotated"))
  );
}
