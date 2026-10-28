use super::remove_fix;

#[test]
fn remove_fix_uses_muse_or_toolu_plugins() {
  assert_eq!(remove_fix("jev", "muse"), "muse plugins remove jev");
  assert_eq!(
    remove_fix("jev", "claude"),
    "toolu plugins remove jev --host claude --yes"
  );
}
