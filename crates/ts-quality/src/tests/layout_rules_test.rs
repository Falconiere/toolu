use std::path::Path;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;
use toolu_state::detect::project::NodePackageManager;

use super::{declares_at_alias, parent_import, test_placement, tests_parent};
use crate::file::{Limits, Place, TsFile};

fn put(root: &Path, rel: &str, body: &str) {
  let path = root.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}

/// The layout rules for `path` under `root`, the hook running at `root`.
fn layout(root: &Path, path: &str, text: &str) -> (Option<String>, Vec<String>) {
  let env = Env::default();
  let file = EditedFile {
    path: path.to_owned(),
    absolute: root.join(path),
    removed: false,
  };
  let at = Place {
    env: &env,
    project_root: root,
    cwd: root,
  };
  let f = TsFile::new(&file, text, at, Limits::default(), NodePackageManager::Bun);
  (parent_import(&f), test_placement(&f))
}

#[test]
fn an_alias_needs_an_at_slash_path_key() {
  let dir = tempfile::tempdir().unwrap();
  let config = dir.path().join("tsconfig.json");
  for (body, alias) in [
    (r#"{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}"#, true),
    (
      r#"{"compilerOptions":{"paths":{"~/*":["./src/*"]}}}"#,
      false,
    ),
    (r#"{"compilerOptions":{"paths":[1]}}"#, false),
    ("not json", false),
  ] {
    std::fs::write(&config, body).unwrap();
    assert_eq!(declares_at_alias(&config), alias, "{body}");
  }
  assert!(!declares_at_alias(&dir.path().join("missing.json")));
}

#[test]
fn parent_imports_fail_only_with_an_alias_up_to_the_root() {
  let dir = tempfile::tempdir().unwrap();
  let import = "import { a } from \"../a\";\n";
  assert_eq!(layout(dir.path(), "src/b.ts", import).0, None);
  put(
    dir.path(),
    "tsconfig.base.json",
    r#"{"compilerOptions":{"paths":{"@/x":["./x"]}}}"#,
  );
  assert_eq!(
    layout(dir.path(), "src/deep/b.ts", import).0.as_deref(),
    Some("Forbidden ../ import in src/deep/b.ts — use @/ alias")
  );
  let sibling = "import { a } from \"./a\";\n";
  assert_eq!(layout(dir.path(), "src/b.ts", sibling).0, None);
}

#[test]
fn test_files_live_flat_beside_their_code() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  assert_eq!(
    layout(root, "src/a.test.ts", "").1,
    ["Test file outside __tests__/: src/a.test.ts — move to sibling __tests__/ directory"]
  );
  assert_eq!(layout(root, "app/e2e/a.test.ts", "").1.len(), 0);
  assert_eq!(layout(root, "src/a.ts", "").1.len(), 0);
  put(root, "src/a.ts", "");
  assert_eq!(layout(root, "src/__tests__/a.test.ts", "").1.len(), 0);
  assert_eq!(
    layout(root, "src/__tests__/helpers/a.test.ts", "").1.len(),
    0
  );
  let nested = layout(root, "src/__tests__/unit/a.test.ts", "").1;
  assert_eq!(nested.len(), 1);
  assert!(nested[0].starts_with("Test nested in __tests__/ subdirectory"));
  let lonely = layout(root, "lib/__tests__/a.test.ts", "").1;
  assert_eq!(
    lonely,
    [
      "Test not co-located with source: lib/__tests__/a.test.ts — __tests__/ must be at the same level as the code it tests"
    ]
  );
  put(root, "pkg/sub/x.md", "");
  assert_eq!(layout(root, "pkg/__tests__/a.test.ts", "").1.len(), 0);
}

#[test]
fn the_tests_parent_follows_dirname() {
  assert_eq!(tests_parent("src/"), "src");
  assert_eq!(tests_parent(""), ".");
  assert_eq!(tests_parent("/"), "/");
}
