use std::path::Path;

use toolu_runtime::process::{Spec, run};

use super::{agent_name, cksum, gh_config, group_file, native_worktree, shell_quote};

fn posix_cksum(text: &str) -> u32 {
  let mut spec = Spec::new(["cksum"]);
  spec.stdin = text.as_bytes().to_vec();
  let output = run(&spec).unwrap();
  output
    .stdout
    .split_whitespace()
    .next()
    .unwrap()
    .parse()
    .unwrap()
}

#[test]
fn fixer_names_match_posix_cksum() {
  for slot in [
    "falconiere-toolu-165",
    "falconiere-comemory-42",
    "",
    &"x".repeat(300),
  ] {
    assert_eq!(cksum(slot.as_bytes()), posix_cksum(slot), "{slot}");
  }
  let sum = posix_cksum("falconiere-toolu-165") % 16_777_216;
  assert_eq!(
    agent_name("falconiere-toolu-165", 2, 1),
    format!("pb-{sum:06x}-r2g1")
  );
}

#[test]
fn group_files_sit_beside_the_state() {
  let state = Path::new("/t/owner's run/falconiere-toolu-165.json");
  let report = group_file(state, 1, 2, ".report.json");
  assert_eq!(
    report,
    Path::new("/t/owner's run/falconiere-toolu-165.fixer-r1g2.report.json")
  );
  assert_eq!(
    gh_config(state),
    Path::new("/t/owner's run/falconiere-toolu-165.fixer-gh")
  );
  assert_eq!(
    native_worktree(state),
    Path::new("/t/owner's run/falconiere-toolu-165.worktree")
  );
  assert_eq!(shell_quote("owner's run"), "'owner'\\''s run'");
}
