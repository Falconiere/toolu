use std::path::Path;

use toolu_runtime::cli::Ctx;

use super::{PLUGIN, QUALITY_PLUGIN, command, run};

#[test]
fn the_python_quality_crate_is_its_plugin_and_prints_its_guide() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(QUALITY_PLUGIN.spec, "python-quality@toolu");
  assert_eq!(command().get_name(), "python-quality");
  let matches = command().try_get_matches_from(["python-quality"]).unwrap();
  let guide = run(&matches, &Ctx::default()).stdout.unwrap_or_default();
  assert!(guide.starts_with("python-quality has no verbs"), "{guide}");
  assert!(guide.contains("lang.python in toolu.config.json"));
  assert!(
    guide.ends_with("Run toolu doctor to see whether the rule is registered and compiled in.")
  );
}
