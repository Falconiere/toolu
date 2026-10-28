use std::path::PathBuf;

use toolu_protocol::host::Host;

use super::plugins_dir;
use crate::env::Env;
use crate::host::roots::Roots;

fn roots(pairs: &[(&str, &str)]) -> Roots {
  let owned: Vec<(String, String)> = pairs
    .iter()
    .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
    .collect();
  Roots::new(Env::from_pairs(owned), Some(Host::Opencode))
}

#[test]
fn plugins_sit_under_the_opencode_config_root() {
  assert_eq!(
    plugins_dir(&roots(&[("HOME", "/home/dev")])),
    PathBuf::from("/home/dev/.config/opencode/plugins")
  );
  assert_eq!(
    plugins_dir(&roots(&[
      ("HOME", "/home/dev"),
      ("XDG_CONFIG_HOME", "/xdg")
    ])),
    PathBuf::from("/xdg/opencode/plugins")
  );
}

#[test]
fn toolu_config_dir_moves_the_plugins_too() {
  let roots = roots(&[("HOME", "/home/dev"), ("TOOLU_CONFIG_DIR", "/tmp/cfg")]);
  assert_eq!(plugins_dir(&roots), PathBuf::from("/tmp/cfg/plugins"));
}
