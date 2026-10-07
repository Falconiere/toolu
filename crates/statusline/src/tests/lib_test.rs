use std::path::Path;

use serde_json::Value;
use toolu_engine::LinkError;
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::cli::Ctx;
use toolu_runtime::host::roots::Roots;

use super::{PLUGIN, command, run};

/// A snapshot the placeholder never asks for.
struct Unported;

impl StatusSnapshot for Unported {
  fn snapshot(&self, _roots: &Roots, _dir: &Path) -> Result<Value, LinkError> {
    Err(LinkError::NotPorted { issue: 445 })
  }
}

#[test]
fn the_statusline_crate_is_its_plugin_and_lists_the_planned_verbs() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let matches = command()
    .try_get_matches_from(["statusline", "planned"])
    .unwrap();
  let outcome = run(&matches, &Ctx::default(), &Unported);
  assert_eq!(
    outcome.stdout.as_deref(),
    Some("toolu statusline is not ported yet (#431). Planned verbs: render, setup, refresh")
  );
}
