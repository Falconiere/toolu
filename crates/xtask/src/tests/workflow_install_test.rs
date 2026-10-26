use super::install_channels;
use crate::workflow_ci::Docs;
use crate::workflow_yaml::Findings;

#[test]
fn without_install_sh_and_workflows_every_channel_check_fails() {
  let root = tempfile::tempdir().unwrap();
  let mut found = Findings::default();
  install_channels(root.path(), &Docs(&[]), &mut found);
  for line in [
    "install.sh must pin the minisign linux archive digest",
    "release-native.yml must upload SHA256SUMS.minisig",
    "release-native.yml must package the tagged install.sh and upload it, because get.toolu.sh serves it from the latest release",
    "release-please.yml must pass the Homebrew App secrets explicitly",
    "release-homebrew.yml must mint a token scoped to homebrew-tap",
  ] {
    assert!(found.0.contains(&line.to_owned()), "{line}: {:?}", found.0);
  }
}
