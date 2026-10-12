use std::path::Path;

use serde_json::Value;
use toolu_runtime::cli::Ctx;

use super::{PLUGIN, QUALITY_PLUGIN, SPEC, command, run};

#[test]
fn the_rust_quality_crate_is_its_plugin_and_prints_its_guide() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  assert_eq!(SPEC, "rust-quality@toolu");
  assert_eq!(
    (QUALITY_PLUGIN.path_glob, QUALITY_PLUGIN.guard),
    ("*.rs", "TOOLU_RUST_QUALITY_NATIVE")
  );
  let bare = command().try_get_matches_from(["rust-quality"]).unwrap();
  let guide = run(&bare, &Ctx::default()).stdout.unwrap_or_default();
  assert!(
    guide.starts_with("rust-quality's rules run after every edit of a .rs file"),
    "{guide}"
  );
  assert!(
    guide.contains("lang.rust in toolu.config.json") && guide.contains("toolu rust-quality check")
  );
  let json = Ctx {
    json: true,
    ..Ctx::default()
  };
  let doc: Value = serde_json::from_str(&run(&bare, &json).stdout.unwrap()).unwrap();
  assert_eq!(
    (doc["namespace"].as_str(), doc["text"].as_str()),
    (Some(PLUGIN), Some(guide.as_str()))
  );
}

#[test]
fn check_takes_any_number_of_paths() {
  let matches = command()
    .try_get_matches_from(["rust-quality", "check", "crates", "src"])
    .unwrap();
  let (verb, check) = matches.subcommand().unwrap();
  let paths: Vec<&str> = check
    .get_many::<String>("path")
    .unwrap()
    .map(String::as_str)
    .collect();
  assert_eq!((verb, paths), ("check", vec!["crates", "src"]));
  assert!(
    command()
      .try_get_matches_from(["rust-quality", "nope"])
      .is_err()
  );
}
