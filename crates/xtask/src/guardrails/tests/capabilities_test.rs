use super::check;
use crate::guardrails::tests::{context, member, rules, tree, tree_with};

fn found_in(files: &[(&str, &str)]) -> Vec<String> {
  let members = vec![
    member("demo", "crates/demo", true),
    member("toolu-runtime", "crates/core/runtime", true),
    member("xtask", "crates/xtask", false),
  ];
  let tree = tree_with(members, files);
  check(&context(&tree.workspace))
    .into_iter()
    .map(|finding| format!("{}:{} {}", finding.path, finding.line, finding.message))
    .collect()
}

#[test]
fn env_process_and_stdio_outside_their_owners_fail_with_the_line() {
  let source = "//! d\nuse std::process::Command;\nfn f() {\n  let _ = std::env::var(\"X\");\n  let _ = std::io::stderr();\n}\n";
  let found = found_in(&[("crates/demo/src/lib.rs", source)]);
  assert_eq!(
    found,
    [
      "crates/demo/src/lib.rs:4 `env::var` is the `env` capability; only toolu-runtime, xtask may use it",
      "crates/demo/src/lib.rs:2 `process::Command` is the `process` capability; only toolu-runtime::process, xtask may use it",
      "crates/demo/src/lib.rs:5 `io::stderr` is the `stdio` capability; only toolu-protocol, toolu-cli::output, xtask::output may use it",
    ]
  );
}

#[test]
fn owners_tests_and_module_owners_pass() {
  let env = "//! r\nfn f() -> bool {\n  std::env::var_os(\"X\").is_some()\n}\n";
  let process = "//! p\nuse std::process::{self, Command};\n";
  let stdout = "//! o\nuse std::io::{Write, stdout};\n";
  let found = found_in(&[
    ("crates/core/runtime/src/lib.rs", env),
    ("crates/core/runtime/src/process.rs", process),
    ("crates/core/runtime/src/process/spawn.rs", process),
    ("crates/xtask/src/output.rs", stdout),
    ("crates/demo/src/tests/lib_test.rs", env),
    ("crates/demo/tests/black_box.rs", process),
  ]);
  assert_eq!(found, Vec::<String>::new(), "{found:#?}");
}

#[test]
fn a_module_owner_does_not_cover_its_siblings() {
  let process = "//! p\nfn f() {\n  let _ = std::process::Command::new(\"git\");\n}\n";
  let found = found_in(&[("crates/core/runtime/src/config.rs", process)]);
  assert_eq!(found.len(), 1);
  assert!(found[0].contains("only toolu-runtime::process, xtask"));
  let xtask_stdio = found_in(&[("crates/xtask/src/gate.rs", "//! g\nuse std::io::stdout;\n")]);
  assert!(xtask_stdio[0].contains("xtask::output"));
}

#[test]
fn a_file_outside_any_member_is_not_production_code() {
  let tree = tree(&[("crates/loose/src/lib.rs", "//! l\nuse std::env::var;\n")]);
  assert_eq!(rules(&check(&context(&tree.workspace))), Vec::<&str>::new());
}

#[test]
fn a_module_alias_or_glob_import_is_a_use_of_the_capability() {
  let found = found_in(&[
    ("crates/demo/src/a.rs", "//! a\nuse std::process as p;\n"),
    ("crates/demo/src/b.rs", "//! b\nuse std::env::*;\n"),
    (
      "crates/demo/src/c.rs",
      "//! c\nuse std::io::{self as stdio, Write};\n",
    ),
    (
      "crates/demo/src/d.rs",
      "//! d\nuse std::env::args as words;\nuse std::fmt::*;\n",
    ),
  ]);
  assert_eq!(
    found,
    [
      "crates/demo/src/a.rs:2 `process::*` is the `process` capability; only toolu-runtime::process, xtask may use it",
      "crates/demo/src/b.rs:2 `env::*` is the `env` capability; only toolu-runtime, xtask may use it",
      "crates/demo/src/c.rs:2 `io::*` is the `stdio` capability; only toolu-protocol, toolu-cli::output, xtask::output may use it",
    ]
  );
}
