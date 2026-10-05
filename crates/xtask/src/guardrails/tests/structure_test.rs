use super::check;
use crate::guardrails::tests::{context, member, rules, tree, tree_with};

fn messages(found: &[crate::guardrails::Finding]) -> Vec<String> {
  found
    .iter()
    .map(|finding| format!("{}: {}", finding.path, finding.message))
    .collect()
}

#[test]
fn the_repository_folders_pass_and_a_stray_root_entry_fails() {
  let mut files = vec![
    ("Cargo.toml", ""),
    ("crates/demo/Cargo.toml", ""),
    ("crates/demo/src/lib.rs", "//! d\n"),
  ];
  let ok = tree(&files);
  let mut ctx = context(&ok.workspace);
  ctx.folders.crates.push("demo".to_owned());
  assert_eq!(check(&ctx), Vec::new(), "{:?}", check(&ctx));
  files.push(("stray.txt", "x"));
  let stray = tree(&files);
  let mut ctx = context(&stray.workspace);
  ctx.folders.crates.push("demo".to_owned());
  assert_eq!(
    messages(&check(&ctx)),
    ["stray.txt: `stray.txt` is not in the folder allowlist for `.`"]
  );
}

#[test]
fn crates_core_crate_and_plugin_entries_are_allowlisted() {
  let members = vec![
    member("toolu-x", "crates/core/x", true),
    member("demo", "crates/demo", true),
  ];
  let tree = tree_with(
    members,
    &[
      ("crates/core/x/Cargo.toml", ""),
      ("crates/demo/benches/b.rs", "//! b\n"),
      ("crates/other/Cargo.toml", ""),
      ("plugins/p/bin/tool", ""),
      ("plugins/p/skills/s/SKILL.md", ""),
    ],
  );
  let mut ctx = context(&tree.workspace);
  ctx.folders.crates.push("demo".to_owned());
  let found = messages(&check(&ctx));
  assert_eq!(
    found,
    [
      "crates/core/x: `x` is not in the folder allowlist for `crates/core`",
      "crates/demo/benches: `benches` is not in the folder allowlist for `crates/demo`",
      "crates/other: `other` is not in the folder allowlist for `crates`",
      "plugins/p/bin: `bin` is not in the folder allowlist for `plugins/p`",
    ]
  );
}

#[test]
fn rust_file_names_depth_main_and_build_are_checked() {
  let tree = tree(&[
    ("crates/demo/src/BadName.rs", "//! b\n"),
    ("crates/demo/src/inner/mod.rs", "//! m\n"),
    ("crates/demo/src/a/b/c/d/deep.rs", "//! d\n"),
    ("crates/demo/src/a/b/c/ok.rs", "//! o\n"),
    ("crates/demo/src/main.rs", "//! m\nfn main() {}\n"),
    ("crates/demo/build.rs", "fn main() {}\n"),
  ]);
  let mut ctx = context(&tree.workspace);
  ctx.folders.crates.push("demo".to_owned());
  let found: Vec<String> = check(&ctx).into_iter().map(|f| f.message).collect();
  assert!(found.contains(&"`BadName.rs` is not a snake_case file name".to_owned()));
  assert!(found.iter().any(|m| m.starts_with("a mod file")));
  assert!(found.contains(&"4 directory levels under src, limit 3".to_owned()));
  assert!(found.contains(&"main.rs outside cli, xtask".to_owned()));
  assert!(found.iter().any(|m| m.starts_with("build.rs is reserved")));
  assert!(!found.iter().any(|m| m.contains("ok.rs")));
}

#[test]
fn main_rs_is_allowed_in_the_main_crates_and_fuzz_entries_are_allowlisted() {
  let members = vec![member("xtask", "crates/xtask", false)];
  let tree = tree_with(
    members,
    &[
      ("crates/xtask/src/main.rs", "//! m\n"),
      ("crates/xtask/fuzz/fuzz_targets/f.rs", "//! f\n"),
      ("crates/xtask/fuzz/corpus/seed", ""),
    ],
  );
  let found = check(&context(&tree.workspace));
  assert_eq!(
    messages(&found),
    ["crates/xtask/fuzz/corpus: `corpus` is not in the folder allowlist for `crates/xtask/fuzz`"]
  );
  assert_eq!(rules(&found), ["structure"]);
}
