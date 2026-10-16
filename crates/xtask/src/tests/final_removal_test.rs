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
fn tracked_typescript_bun_and_shell_files_are_findings() {
  let files: Vec<String> = [
    "install.sh",
    "benchmarks/fixtures/sample.ts",
    "tooling/src/x.ts",
    "a/b.mts",
    "bun.lock",
    "nested/bunfig.toml",
    "plugins/jev/scripts/jev.sh",
    "crates/x/src/lib.rs",
  ]
  .map(str::to_owned)
  .to_vec();
  assert_eq!(
    super::tracked_problems(&files),
    [
      "tracked TypeScript file: tooling/src/x.ts",
      "tracked TypeScript file: a/b.mts",
      "tracked Bun file: bun.lock",
      "tracked Bun file: nested/bunfig.toml",
      "tracked shell file: plugins/jev/scripts/jev.sh",
    ]
  );
}

#[test]
fn a_shellcheckrc_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  minimal(root);
  write(root, ".shellcheckrc", "\n");
  let found = super::problems(root).unwrap();
  assert_eq!(found, [".shellcheckrc remains"]);
}

#[test]
fn bun_in_a_hook_or_a_workflow_and_leftover_trees_are_findings() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  minimal(root);
  write(
    root,
    "plugins/x/hooks/hooks.json",
    "{\"command\":\"bun x.js\"}\n",
  );
  write(root, "plugins/x/hooks/dist/a.js", "\n");
  write(root, "packages/core/package.json", "{}\n");
  write(
    root,
    ".github/workflows/other.yml",
    "steps:\n  - uses: oven-sh/setup-bun@v2\n",
  );
  write(root, "package.json", "{\"workspaces\":[\"a\"]}\n");
  let found = super::problems(root).unwrap();
  for line in [
    "packages remains",
    "plugins/x/hooks/dist remains",
    "plugins/x/hooks/hooks.json still names Bun",
    "package.json still declares workspaces",
    "workflow other.yml still uses Bun",
  ] {
    assert!(found.contains(&line.to_owned()), "{line}: {found:?}");
  }
}

#[test]
fn a_bun_bundle_inventory_row_is_a_finding() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  minimal(root);
  write(
    root,
    "fixtures/gate-coverage/inventory.json",
    "[{\"id\":\"demo\",\"classification\":\"port-native\",\"hostMechanism\":\"bun-bundle\",\"implementationStatus\":\"done\",\"bashRequired\":false}]\n",
  );
  let found = super::problems(root).unwrap();
  assert!(
    found.contains(&"demo: hostMechanism bun-bundle".to_owned()),
    "{found:?}"
  );
}

#[test]
fn a_typescript_job_is_a_retired_job() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  minimal(root);
  write(
    root,
    ".github/workflows/tests.yml",
    "jobs:\n  gate:\n  typescript:\n",
  );
  let found = super::problems(root).unwrap();
  assert_eq!(found, ["retired CI job remains: typescript"]);
}

/// A git repository that passes: a package manifest, the `gate` job and one native row.
fn minimal(root: &std::path::Path) {
  assert!(
    Command::new("git")
      .arg("init")
      .current_dir(root)
      .output()
      .unwrap()
      .status
      .success()
  );
  write(root, "package.json", "{\"private\":true}\n");
  write(root, ".github/workflows/tests.yml", "jobs:\n  gate:\n");
  write(
    root,
    "fixtures/gate-coverage/inventory.json",
    "[{\"id\":\"demo\",\"classification\":\"port-native\",\"hostMechanism\":\"native\",\"implementationStatus\":\"done\",\"bashRequired\":false}]\n",
  );
  write(
    root,
    "docs/gate-coverage-matrix.md",
    "| `demo` | `src` | demo | PreToolUse | port-native | required | #1/done | native | no | — |\n",
  );
}

fn write(root: &std::path::Path, rel: &str, body: &str) {
  let path = root.join(rel);
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(path, body).unwrap();
}

#[test]
fn an_unreadable_hooks_json_is_an_error_not_a_pass() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  minimal(root);
  std::fs::create_dir_all(root.join("plugins/x/hooks/hooks.json")).unwrap();
  let error = super::problems(root).unwrap_err();
  assert!(
    error.starts_with("cannot read ") && error.contains("plugins/x/hooks/hooks.json"),
    "{error}"
  );
}
