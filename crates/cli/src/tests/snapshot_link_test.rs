use std::path::Path;

use toolu_engine::status::StatusSnapshot;
use toolu_hub::status::Snapshot;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

#[test]
fn statusline_gets_the_hubs_status_snapshot() {
  let roots = Roots::new(Env::default(), None);
  let document = Snapshot
    .snapshot(&roots, Path::new("/repo"))
    .expect("ported snapshot");
  assert_eq!(
    document.get("namespace").and_then(|value| value.as_str()),
    Some("status")
  );
}
