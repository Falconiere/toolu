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
