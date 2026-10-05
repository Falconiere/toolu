use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, command, run};

#[test]
fn the_delivery_flow_crate_is_its_plugin_and_prints_its_guide() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command().try_get_matches_from(["delivery-flow"]).unwrap();
  let guide = run(&matches, &Ctx::default()).stdout.unwrap();
  assert!(
    guide.starts_with("delivery-flow is a Markdown-only plugin: it has no verbs."),
    "{guide}"
  );
  assert!(guide.contains("/delivery-flow:delivery-flow"), "{guide}");
}
