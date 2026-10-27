use toolu_protocol::host::Host;

use super::{redact_window, redaction_overlap};
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
