//! Test layout, the port of 20-tests: inline `#[cfg(test)]` under src/ (a bare
//! one wiring a bodyless `mod name;` is exempt), then where a test file lives.
//! [`is_rust_test`] is the `_is_rust_test` flag the size, error and docs rules
//! also read.

use std::sync::LazyLock;

use crate::file::{RsFile, Rx, hit, rx, test_file_name};

const MOD_DECL: &str =
  r"(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*;[[:space:]]*(//.*)?$";

static CFG_COMBINATOR: Rx =
  LazyLock::new(|| rx(r"^[[:space:]]*#\[cfg\((all|any)\(test([^A-Za-z0-9_]|$)"));
static CFG_TEST_WITH_DECL: Rx = LazyLock::new(|| {
  rx(&format!(
    r"^[[:space:]]*#\[cfg\(test\)\][[:space:]]*{MOD_DECL}"
  ))
});
static CFG_TEST_ALONE: Rx = LazyLock::new(|| rx(r"^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$"));
static CFG_TEST: Rx = LazyLock::new(|| rx(r"^[[:space:]]*#\[cfg\(test\)\]"));
static OUTER_ATTR: Rx = LazyLock::new(|| rx(r"^[[:space:]]*#\[[^!]"));
static BODYLESS_DECL: Rx = LazyLock::new(|| rx(&format!("^[[:space:]]*{MOD_DECL}")));
static TEST_ATTR: Rx = LazyLock::new(|| {
  rx(r"^[[:space:]]*#\[([A-Za-z_][A-Za-z0-9_]*::)*(test|test_case)\b|^[[:space:]]*#\[rstest\b")
});

/// 8 attributes plus the decl line itself.
const DECL_LOOKAHEAD: usize = 9;

/// After a bare `#[cfg(test)]`: whether the next line past its outer attributes
/// is a bodyless `mod` decl. Consumes the lines it reads.
fn wires_bodyless_decl<'a>(rows: &mut impl Iterator<Item = &'a &'a str>) -> bool {
  let mut decl = None;
  for next in rows.take(DECL_LOOKAHEAD) {
    decl = Some(*next);
    if !hit(&OUTER_ATTR, next) {
      break;
    }
  }
  decl.is_some_and(|decl| hit(&BODYLESS_DECL, decl))
}

/// The awk scan: lines a bare `#[cfg(test)]` consumes looking for its decl are
/// not re-examined.
fn has_inline_cfg_test(lines: &[&str]) -> bool {
  let mut rows = lines.iter();
  while let Some(line) = rows.next() {
    if hit(&CFG_COMBINATOR, line) {
      return true;
    }
    if hit(&CFG_TEST_WITH_DECL, line) {
      continue;
    }
    if hit(&CFG_TEST_ALONE, line) {
      if wires_bodyless_decl(&mut rows) {
        continue;
      }
      return true;
    }
    if hit(&CFG_TEST, line) {
      return true;
    }
  }
  false
}

/// `_is_rust_test`: a `_test.rs`/`_tests.rs` name, or a test attribute in the body.
pub(crate) fn is_rust_test(f: &RsFile<'_>) -> bool {
  test_file_name(f.path()) || f.lines.iter().any(|line| hit(&TEST_ATTR, line))
}

const ALLOWED_SUBDIRS: [&str; 3] = ["fixtures", "helpers", "common"];

/// 20-tests.
pub(crate) fn test_layout(f: &RsFile<'_>, is_test: bool) -> Option<String> {
  let path = f.path();
  if f.path_has("/src/") && has_inline_cfg_test(&f.lines) {
    return Some(format!(
      "Inline #[cfg(test)] in {path} — unit tests belong in a module-sibling tests/ dir wired by a bodyless #[cfg(test)] mod tests; decl; crate-root tests/ is for integration tests"
    ));
  }
  if !is_test {
    return None;
  }
  let Some((_, after)) = f.scope.rsplit_once("/tests/") else {
    return Some(format!(
      "Rust test file outside tests/: {path} — move to a sibling tests/ directory"
    ));
  };
  let (subdir, _) = after.split_once('/')?;
  (!ALLOWED_SUBDIRS.contains(&subdir)).then(|| {
    format!(
      "Rust test nested in tests/ subdirectory: {path} — keep tests/ flat (only fixtures/helpers/common subdirs allowed)"
    )
  })
}

#[cfg(test)]
#[path = "tests/layout_rules_test.rs"]
mod tests;
