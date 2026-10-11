use std::process::Command;

use crate::Verdict;
use crate::task_options::{test_options, test_root};

#[test]
fn this_repository_passes() {
  assert_eq!(
    super::run(&test_options(test_root())).unwrap(),
    Verdict::Clean
  );
}

#[test]
fn native_shims_are_kept() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  assert!(
    Command::new("git")
      .arg("init")
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
  for shim in [
    "plugins/jev/scripts/jev.sh",
    "plugins/statusline/scripts/statusline.sh",
    "plugins/toolu-review/scripts/write-state.sh",
  ] {
    write(root, shim, "#!/bin/sh\nexec toolu\n");
  }
  write(root, "plugins/x/scripts/run.sh", "#!/bin/sh\n");
  assert!(
    Command::new("git")
      .args(["add", "."])
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
  assert_eq!(
    super::tracked_shell(root).unwrap(),
    vec!["plugins/x/scripts/run.sh".to_owned()]
  );
}

#[test]
fn a_shellcheckrc_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  assert!(
    Command::new("git")
      .arg("init")
      .current_dir(root)
      .status()
      .unwrap()
      .success()
  );
  write(root, "package.json", "{\"scripts\":{}}\n");
  write(
    root,
    ".github/workflows/tests.yml",
    "jobs:\n  typescript:\n",
  );
  write(
    root,
    "fixtures/gate-coverage/inventory.json",
    "[{\"id\":\"demo\",\"classification\":\"port-native\",\"hostMechanism\":\"bun-bundle\",\"implementationStatus\":\"done\",\"bashRequired\":false}]\n",
  );
  write(
    root,
    "docs/gate-coverage-matrix.md",
    "| `demo` | `src` | demo | PreToolUse | port-native | required | #1/done | bun-bundle | no | — |\n",
  );
  write(root, ".shellcheckrc", "\n");
  let found = super::problems(root).unwrap();
  assert!(
    found
      .iter()
      .any(|line| line.contains(".shellcheckrc remains")),
    "{found:?}"
  );
}

fn write(root: &std::path::Path, rel: &str, body: &str) {
  let path = root.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}
