use std::path::Path;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;

use super::{has_inline_cfg_test, is_rust_test, test_layout};
use crate::file::{Limits, RsFile};

/// `(is_rust_test, test_layout)` of `path` holding `lines`.
fn layout(path: &str, lines: &[&str]) -> (bool, Option<String>) {
  let file = EditedFile {
    path: path.to_owned(),
    absolute: Path::new(path).into(),
    removed: false,
  };
  let (limits, env) = (Limits::default(), Env::default());
  let scope = format!("/{path}");
  let f = RsFile {
    file: &file,
    scope: &scope,
    lines: lines.to_vec(),
    limits: &limits,
    env: &env,
    cwd: Path::new("/"),
  };
  let is_test = is_rust_test(&f);
  (is_test, test_layout(&f, is_test))
}

#[test]
fn a_bare_cfg_test_wiring_a_bodyless_mod_is_allowed() {
  let wired = [
    "#[cfg(test)]",
    "#[path = \"tests/a_test.rs\"]",
    "mod tests;",
  ];
  assert!(!has_inline_cfg_test(&wired));
  assert!(!has_inline_cfg_test(&[
    "#[cfg(test)] pub mod tests; // unit tests"
  ]));
  assert!(has_inline_cfg_test(&["#[cfg(test)]", "mod tests {", "}"]));
  assert!(
    has_inline_cfg_test(&["#[cfg(test)]"]),
    "a file ending in the attribute"
  );
  assert!(has_inline_cfg_test(&["#[cfg(test)] fn helper() {}"]));
  assert!(has_inline_cfg_test(&["#[cfg(all(test, unix))]"]));
  let nine_attrs: Vec<&str> = std::iter::once("#[cfg(test)]")
    .chain(std::iter::repeat_n("#[a]", 9))
    .chain(["mod tests;"])
    .collect();
  assert!(
    has_inline_cfg_test(&nine_attrs),
    "the decl lies past the lookahead"
  );
}

#[test]
fn inline_tests_under_src_are_reported() {
  let (_, found) = layout("crates/x/src/lib.rs", &["#[cfg(test)]", "mod tests {}"]);
  assert_eq!(
    found.as_deref(),
    Some(
      "Inline #[cfg(test)] in crates/x/src/lib.rs — unit tests belong in a module-sibling tests/ dir wired by a bodyless #[cfg(test)] mod tests; decl; crate-root tests/ is for integration tests"
    )
  );
  assert_eq!(
    layout("build.rs", &["#[cfg(test)]", "mod tests {}"]).1,
    None
  );
}

#[test]
fn test_files_belong_in_a_flat_tests_dir() {
  let (is_test, found) = layout("src/a.rs", &["  #[tokio::test]", "async fn t() {}"]);
  assert!(is_test);
  assert_eq!(
    found.as_deref(),
    Some("Rust test file outside tests/: src/a.rs — move to a sibling tests/ directory")
  );
  assert!(layout("src/b.rs", &["#[rstest]"]).0 && !layout("src/c.rs", &["#[testing]"]).0);
  assert_eq!(layout("crates/x/src/tests/a_test.rs", &[]), (true, None));
  assert_eq!(
    layout("tests/helpers/a_test.rs", &[]),
    (true, None),
    "a top-level tests/"
  );
  assert_eq!(
    layout("crates/x/tests/deep/a_test.rs", &[]).1.as_deref(),
    Some(
      "Rust test nested in tests/ subdirectory: crates/x/tests/deep/a_test.rs — keep tests/ flat (only fixtures/helpers/common subdirs allowed)"
    )
  );
  assert_eq!(layout("src/lib.rs", &["fn a() {}"]), (false, None));
}
