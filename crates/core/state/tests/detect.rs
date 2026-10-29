//! Detection against real repositories and real `PATH` directories (AC-12): the 35
//! layouts of the former `detect-project.test.ts` built in git repositories, and the
//! cases of `detect-tools.test.ts`. The line counters' parity run against
//! `detect-lines.ts` was retired with the TypeScript tree (#440); their unit tests stay.

#[path = "helpers/layouts.rs"]
mod layouts;
#[path = "helpers/scratch.rs"]
mod scratch;

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use layouts::Layout;
use scratch::{Res, Scratch, mkdir, write};
use toolu_runtime::env::Env;
use toolu_state::detect::project::{
  NodePackageManager, PythonLinter, TsLinter, detect_clippy, detect_python, detect_rust, detect_ts,
  node_package_manager, project_name, project_toplevel, python_linter, to_relative_path, ts_linter,
};
use toolu_state::detect::tools::{detect_ast_grep, tool_available};

const TRUE: &str = "/usr/bin/true";

/// Every project probe from `cwd`, in a stable order: toplevel, name, node package
/// manager, rust, python, ts, ts linter, python linter, clippy.
fn probe(env: &Env, cwd: &Path) -> Vec<String> {
  let flag = |on: bool, word: &str| if on { word.to_owned() } else { String::new() };
  vec![
    project_toplevel(env, cwd)
      .map(|root| root.display().to_string())
      .unwrap_or_default(),
    project_name(env, cwd).unwrap_or_default(),
    node_package_manager(env, cwd)
      .map_or("", NodePackageManager::name)
      .to_owned(),
    flag(detect_rust(env, cwd), "rust"),
    flag(detect_python(env, cwd), "python"),
    flag(detect_ts(env, cwd), "ts"),
    ts_linter(env, cwd).map_or("", TsLinter::name).to_owned(),
    python_linter(env, cwd)
      .map_or("", PythonLinter::name)
      .to_owned(),
    flag(detect_clippy(env, cwd), "clippy"),
  ]
}

/// Where a layout's probes differ from TypeScript's answers or from each other.
fn mismatches(
  layout: &Layout,
  root: &Path,
  at_root: &[String],
  from_sub: &[String],
) -> Vec<String> {
  let name = &layout.name;
  let top = root.display().to_string();
  let mut found = Vec::new();
  if at_root.first() != Some(&top) {
    found.push(format!(
      "{name}: toplevel {:?}, want {top}",
      at_root.first()
    ));
  }
  if at_root.get(1).map(String::as_str) != Some("project") {
    found.push(format!("{name}: project name {:?}", at_root.get(1)));
  }
  if at_root.get(2..) != Some(layout.expect.as_slice()) {
    found.push(format!(
      "{name}: probes {at_root:?}, want {:?}",
      layout.expect
    ));
  }
  if from_sub != at_root {
    found.push(format!(
      "{name}: from a subdirectory {from_sub:?}, from the root {at_root:?}"
    ));
  }
  if let Some((index, answer)) = &layout.bats
    && at_root.get(*index) != Some(answer)
  {
    let got = at_root.get(*index);
    found.push(format!(
      "{name}: bats wants {answer:?} at {index}, got {got:?}"
    ));
  }
  found
}

/// Build every layout in a real repository and collect where its probes differ.
fn layout_mismatches(layouts: &[Layout]) -> Res<Vec<String>> {
  let mut found = Vec::new();
  for layout in layouts {
    let scratch = Scratch::new()?;
    let root = layouts::build(&scratch, layout)?;
    let env = scratch.env()?;
    let sub = root.join("sub/dir");
    mkdir(&sub)?;
    found.extend(mismatches(
      layout,
      &root,
      &probe(&env, &root),
      &probe(&env, &sub),
    ));
  }
  Ok(found)
}

#[test]
fn every_layout_gives_typescripts_answers_from_the_root_and_a_subdirectory() {
  let layouts = layouts::load().unwrap();
  assert_eq!(layouts.len(), 35);
  let found = layout_mismatches(&layouts).unwrap();
  assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn outside_a_repository_every_probe_is_empty() {
  let scratch = Scratch::new().unwrap();
  write(&scratch.root.join("Cargo.toml"), b"").unwrap();
  write(&scratch.root.join("tsconfig.json"), b"{}").unwrap();
  assert_eq!(probe(&scratch.env().unwrap(), &scratch.root).concat(), "");
}

#[test]
fn a_path_is_relative_to_the_toplevel_only_when_under_it() {
  let scratch = Scratch::new().unwrap();
  let project = scratch.root.join("project");
  mkdir(&project).unwrap();
  scratch.git(&project, &["init", "-q"]).unwrap();
  let outside = scratch.root.join("outside");
  mkdir(&outside).unwrap();
  let env = scratch.env().unwrap();
  let top = project.display().to_string();
  let sibling = format!("{top}-sibling/a.ts");
  let inputs = [
    format!("{top}/src/a.ts"),
    format!("{top}/deep/x/y.rs"),
    "src/a.ts".to_owned(),
    "/etc/hosts".to_owned(),
    String::new(),
    top.clone(),
    format!("{top}/"),
    sibling.clone(),
    "a path with spaces/x.ts".to_owned(),
  ];
  let inside: Vec<String> = inputs
    .iter()
    .map(|p| to_relative_path(p, &env, &project))
    .collect();
  let want = [
    "src/a.ts",
    "deep/x/y.rs",
    "src/a.ts",
    "/etc/hosts",
    "",
    top.as_str(),
    "",
  ];
  assert_eq!(inside[..7], want);
  assert_eq!(inside[7..], [sibling.as_str(), "a path with spaces/x.ts"]);
  let elsewhere: Vec<String> = inputs
    .iter()
    .map(|p| to_relative_path(p, &env, &outside))
    .collect();
  assert_eq!(elsewhere, inputs);
}

/// A bin dir `label` under `root` holding real executables under `names`.
fn bin(root: &Path, label: &str, names: &[&str]) -> Res<PathBuf> {
  let dir = root.join(label);
  mkdir(&dir)?;
  for name in names {
    symlink(TRUE, dir.join(name)).map_err(|err| format!("{name}: {err}"))?;
  }
  Ok(dir)
}

/// `PATH` holding only `dir`, which keeps an unrelated system `sg` (shadow-utils) out.
fn only(scratch: &Scratch, dir: &Path) -> Env {
  scratch.env_with(&dir.display().to_string())
}

#[test]
fn tool_available_tells_executables_plain_files_directories_and_missing_names_apart() {
  let scratch = Scratch::new().unwrap();
  let dir = bin(&scratch.root, "bin", &["exe"]).unwrap();
  write(&dir.join("plain"), b"#!/bin/sh\n").unwrap();
  mkdir(&dir.join("adir")).unwrap();
  symlink(scratch.root.join("missing"), dir.join("dangling")).unwrap();
  let env = scratch.env_with(&format!("{}:/usr/bin:/bin", dir.display()));
  let paths = ["exe", "plain", "adir"].map(|name| dir.join(name).display().to_string());
  let names = ["exe", "plain", "adir", "dangling", "nope", "git", ""].map(str::to_owned);
  let found: Vec<bool> = names
    .iter()
    .chain(&paths)
    .map(|n| tool_available(n, &env))
    .collect();
  assert_eq!(
    found,
    [
      true, true, false, false, false, true, false, true, false, false
    ]
  );
}

#[test]
fn an_empty_path_entry_is_the_current_directory() {
  let scratch = Scratch::new().unwrap();
  let dir = bin(&scratch.root, "bin", &[]).unwrap();
  // `cargo test` runs from the package directory, which holds a Cargo.toml.
  for path in [
    String::new(),
    format!("{}:", dir.display()),
    format!(":{}", dir.display()),
  ] {
    assert!(
      tool_available("Cargo.toml", &scratch.env_with(&path)),
      "PATH={path:?}"
    );
  }
  assert!(!tool_available("Cargo.toml", &only(&scratch, &dir)));
}

#[test]
fn the_probe_cache_is_keyed_by_path_so_a_new_path_re_probes() {
  let scratch = Scratch::new().unwrap();
  let without = only(&scratch, &bin(&scratch.root, "without", &[]).unwrap());
  let with = only(&scratch, &bin(&scratch.root, "with", &["sg"]).unwrap());
  assert!(!detect_ast_grep(&without));
  assert!(detect_ast_grep(&with));
  assert!(!detect_ast_grep(&without));
  assert!(!tool_available("sg", &scratch.env_with("")));
}
