use std::fs;

#[test]
fn a_checkout_without_builtins_is_an_error() {
  let dir = std::env::temp_dir().join(format!("gate-cov-{}", std::process::id()));
  fs::create_dir_all(&dir).expect("temp dir");
  let err = super::discover(&dir).expect_err("missing builtins");
  let _ = fs::remove_dir_all(&dir);
  assert!(
    err.contains("cannot read plugins/toolu/hooks/src/pre-tools/builtins.ts"),
    "{err}"
  );
}
