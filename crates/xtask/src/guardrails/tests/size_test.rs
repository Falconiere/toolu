use super::check;
use crate::guardrails::tests::{context, tree};

/// A module with `n` code lines: a doc line and a comment, then `n` constants.
fn module(n: usize) -> String {
  let consts = (0..n)
    .map(|index| format!("const C{index}: u8 = 1;\n"))
    .collect::<Vec<_>>()
    .concat();
  format!("//! demo\n\n// a comment is not code\n{consts}")
}

/// An impl block of `n` code lines.
fn impl_block(n: usize) -> String {
  let consts = (2..n)
    .map(|index| format!("  const C{index}: u8 = 1;\n"))
    .collect::<Vec<_>>()
    .concat();
  format!("struct S;\nimpl S {{\n{consts}}}\n")
}

#[test]
fn a_file_at_the_limit_passes_and_one_over_fails() {
  let at = tree(&[("crates/demo/src/at.rs", &module(300))]);
  assert_eq!(check(&context(&at.workspace)), Vec::new());
  let over = tree(&[("crates/demo/src/over.rs", &module(301))]);
  let found = check(&context(&over.workspace));
  assert_eq!(found.len(), 1);
  assert_eq!(found[0].rule, "file-length");
  assert!(
    found[0].message.starts_with("301 code lines, limit 300"),
    "{}",
    found[0].message
  );
}

#[test]
fn an_impl_block_over_the_limit_fails_with_its_line() {
  let text = format!("//! demo\n{}", impl_block(201));
  let over = tree(&[("crates/demo/src/s.rs", &text)]);
  let found = check(&context(&over.workspace));
  assert_eq!(found.len(), 1);
  assert_eq!((found[0].rule, found[0].line), ("impl-length", 3));
  assert!(found[0].message.contains("201 code lines, limit 200"));
  let at = tree(&[(
    "crates/demo/src/s.rs",
    &format!("//! demo\n{}", impl_block(200)),
  )]);
  assert_eq!(check(&context(&at.workspace)), Vec::new());
}

#[test]
fn an_unparsable_file_is_still_measured() {
  let text = format!("{}fn (", module(301));
  let broken = tree(&[("crates/demo/src/broken.rs", &text)]);
  let found = check(&context(&broken.workspace));
  assert_eq!(found[0].rule, "file-length");
}
