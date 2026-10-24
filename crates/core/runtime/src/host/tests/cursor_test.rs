use std::path::PathBuf;

use toolu_protocol::host::Host;

use super::{cache_dir, local_dir};
use crate::env::Env;
use crate::host::roots::Roots;

fn roots(pairs: &[(&str, &str)]) -> Roots {
  let owned: Vec<(String, String)> = pairs
    .iter()
    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
    .collect();
  Roots::new(Env::from_pairs(owned), Some(Host::Cursor))
}

#[test]
fn plugin_directories_sit_under_the_cursor_config_root() {
  let roots = roots(&[("HOME", "/home/dev")]);
  assert_eq!(
    local_dir(&roots),
    PathBuf::from("/home/dev/.cursor/plugins/local")
  );
  assert_eq!(
    cache_dir(&roots),
    PathBuf::from("/home/dev/.cursor/plugins/cache/toolu")
  );
}

#[test]
fn toolu_config_dir_moves_both() {
  let roots = roots(&[("HOME", "/home/dev"), ("TOOLU_CONFIG_DIR", "/tmp/cfg")]);
  assert_eq!(local_dir(&roots), PathBuf::from("/tmp/cfg/plugins/local"));
}
