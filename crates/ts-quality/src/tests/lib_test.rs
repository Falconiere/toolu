use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, SPEC, command, run};

#[test]
fn the_ts_quality_crate_is_its_plugin_and_prints_its_guide() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(SPEC, "ts-quality@toolu");
  assert_eq!(command().get_name(), "ts-quality");
  let matches = command().try_get_matches_from(["ts-quality"]).unwrap();
  let guide = run(&matches, &Ctx::default()).stdout.unwrap_or_default();
  assert!(guide.starts_with("ts-quality has no verbs"), "{guide}");
  assert!(guide.contains("lang.ts in toolu.config.json"));
  assert!(
    guide.ends_with("Run toolu doctor to see whether the rule is registered and compiled in.")
  );
}
