use toolu_runtime::env::Env;

use super::root;

#[test]
fn root_resolves_the_real_repository_from_the_crate_directory() {
  let env = Env::process();
  let repo = root(&env, env!("CARGO_MANIFEST_DIR")).unwrap();
  assert!(repo.join("Cargo.toml").is_file());
  assert!(repo.join("crates/toolu-review/Cargo.toml").is_file());
}
