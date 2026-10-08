use serde_json::Map;
use toolu_protocol::host::Host;
use toolu_runtime::config::load::LoadedConfig;

use super::diagnostics;

#[test]
fn silence_is_empty_and_warnings_keep_their_order() {
  let quiet = LoadedConfig::from_data(Map::new(), Host::Claude);
  assert_eq!(diagnostics(None, &quiet, Vec::new()), None);
  let config = LoadedConfig::from_data(Map::new(), Host::Claude);
  config.warn("bad envelope".to_owned());
  assert_eq!(
    diagnostics(Some("host".to_owned()), &config, vec!["swept".to_owned()]).as_deref(),
    Some("host\ntoolu-config: bad envelope\nswept")
  );
}
