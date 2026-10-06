use serde_json::{Value, json};
use toolu_protocol::host::Host;

use super::{DocsSyncKey, docs_sync_globs};
use crate::config::load::LoadedConfig;

fn globs(docs: Value, key: DocsSyncKey) -> Vec<String> {
  let mut data = serde_json::Map::new();
  data.insert("docsSync".to_owned(), docs);
  let config = LoadedConfig::from_data(data, Host::Claude);
  docs_sync_globs(&config, key)
}

#[test]
fn an_array_replaces_the_default_list() {
  assert_eq!(
    globs(json!({ "surfaces": ["only.md"] }), DocsSyncKey::Surfaces),
    ["only.md"]
  );
}

#[test]
fn an_empty_or_non_array_override_keeps_the_default() {
  assert_eq!(
    globs(json!({ "surfaces": [] }), DocsSyncKey::Surfaces),
    DocsSyncKey::Surfaces.defaults()
  );
  assert_eq!(
    globs(
      json!({ "surfaceExcludes": "nope" }),
      DocsSyncKey::SurfaceExcludes
    ),
    ["docs/releases/*", "*/docs/releases/*"]
  );
  assert_eq!(globs(json!(null), DocsSyncKey::CodeSurfaces).len(), 6);
  assert_eq!(
    globs(json!({ "codeSurfaces": [""] }), DocsSyncKey::CodeSurfaces).len(),
    6
  );
}

#[test]
fn non_strings_print_as_jq_raw_output_does() {
  let got = globs(
    json!({ "codeSurfaces": ["*.go", 7, { "a": [1] }, "*.zig\n\n"] }),
    DocsSyncKey::CodeSurfaces,
  );
  assert_eq!(
    got,
    ["*.go", "7", "{", "  \"a\": [", "    1", "  ]", "}", "*.zig"]
  );
}
