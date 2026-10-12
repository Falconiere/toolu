use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use serde_json::{Map, json};
use toolu_engine::quality::{AstGrepScan, EditedFile, QualityRule};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::env::Env;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::RuleContext;

use super::{RUST_QUALITY, RustQuality, limits};

/// A Write of `path` under `root`, as the host normalizes it.
fn wrote(root: &Path, path: &str) -> NormalizedEvent {
  let at = root.display().to_string();
  serde_json::from_value(json!({
    "type": "tool/post", "sessionId": "s", "cwd": at, "projectRoot": at, "worktree": at,
    "toolCallId": "t", "toolName": "Write", "toolInput": {"file_path": path},
  }))
  .unwrap()
}

/// An executable `name` in `bin` running `body`.
fn tool(bin: &Path, name: &str, body: &str) {
  std::fs::create_dir_all(bin).unwrap();
  std::fs::write(bin.join(name), format!("#!/bin/sh\n{body}\n")).unwrap();
  std::fs::set_permissions(bin.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A Cargo repository at `root` with a `cargo` stub ahead of the real `PATH`.
fn cargo_repo(root: &Path) -> Env {
  std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
  assert!(
    Command::new("git")
      .arg("-C")
      .arg(root)
      .args(["init", "-q"])
      .status()
      .unwrap()
      .success()
  );
  tool(&root.join("bin"), "cargo", "exit 0");
  let home = root.join("home").display().to_string();
  let process = Env::process();
  let path = format!(
    "{}:{}",
    root.join("bin").display(),
    process.get("PATH").unwrap_or_default()
  );
  process
    .with("PATH", &path)
    .with("HOME", &home)
    .with("TOOLU_CONFIG_DIR", &home)
}

fn decide(env: &Env, root: &Path, path: &str, body: &str) -> Decision {
  std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
  std::fs::write(root.join(path), body).unwrap();
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let event = wrote(root, &root.join(path).display().to_string());
  assert!(RUST_QUALITY.applies(&event, &ctx));
  let mut warnings = Vec::new();
  let decision = RUST_QUALITY.run_warning(&event, &ctx, &mut warnings);
  assert!(warnings.is_empty(), "{warnings:?}");
  decision
}

fn message(decision: Decision) -> String {
  match decision {
    Decision::Advisory { message } => message.as_str().to_owned(),
    other @ (Decision::Allow
    | Decision::Ask { .. }
    | Decision::Deny { .. }
    | Decision::Block { .. }
    | Decision::RuntimeFailure { .. }) => panic!("expected an advisory, got {other:?}"),
  }
}

#[test]
fn the_manifest_enables_the_rule_on_rs_edits_of_cargo_projects() {
  assert_eq!(
    (RUST_QUALITY.spec(), RUST_QUALITY.name()),
    ("rust-quality@toolu", "rust-quality")
  );
  assert_eq!(RUST_QUALITY.event(), RegistryEvent::ToolPost);
  assert_eq!(
    (RustQuality.language(), RustQuality.source()),
    ("rust", "rust-quality-hook")
  );
  assert_eq!(
    RustQuality.reason(),
    "Post-edit Rust quality violation(s) detected"
  );
  assert_eq!(RustQuality.extensions(), ["rs"]);
  assert!(!RustQuality.skip_linked_worktrees() && RustQuality.ast_rule_dirs().is_empty());
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let (env, raw) = (Env::default(), Map::new());
  let ctx = RuleContext {
    host: Host::Codex,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  assert_eq!(RustQuality.ast_rules_inline(&ctx), "");
  assert!(!RUST_QUALITY.applies(&wrote(root, "a.ts"), &ctx));
  assert!(!RustQuality.project_enabled(&ctx), "no Cargo.toml");
  let env = cargo_repo(root);
  let ctx = RuleContext { env: &env, ..ctx };
  assert!(RustQuality.project_enabled(&ctx));
  let without_cargo = env.clone().with("PATH", "/nonexistent");
  assert!(!RustQuality.project_enabled(&RuleContext {
    env: &without_cargo,
    ..ctx
  }));
}

#[test]
fn violations_are_reported_with_real_ast_grep_and_a_clean_file_passes() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let env = cargo_repo(root);
  let bad = "//! A.\n#[allow(dead_code)]\nfn a() -> u8 {\n  Some(1).unwrap()\n}\n";
  let found = message(decide(&env, root, "src/lib.rs", bad));
  let file = root.join("src/lib.rs").display().to_string();
  assert!(
    found.starts_with("QUALITY VIOLATION — fix before proceeding:\nForbidden lint suppression"),
    "{found}"
  );
  assert!(
    found.contains(&format!(
      ".unwrap() in {file} — use ? or match on Result/Option\n{file}:4:  Some(1).unwrap()\n"
    )),
    "{found}"
  );
  let clean = "//! A.\n/// B.\npub fn b() -> u8 {\n  1\n}\n";
  assert_eq!(decide(&env, root, "src/lib.rs", clean), Decision::Allow);
  let undocumented = message(decide(&env, root, "src/lib.rs", "pub fn b() {}\n"));
  assert!(
    undocumented.starts_with("Public items missing a /// doc comment in "),
    "{undocumented}"
  );
}

#[test]
fn a_crashing_ast_grep_fails_each_group_that_applies() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let env = cargo_repo(root);
  tool(&root.join("bin"), "ast-grep", "echo boom >&2; exit 3");
  let found = message(decide(&env, root, "src/lib.rs", "//! A.\n"));
  assert!(found.contains("— ast-grep exit 3: boom; error-handling rules could not be verified."));
  assert!(found.contains("— ast-grep exit 3: boom; no-mocks rules could not be verified."));
  assert_eq!(
    decide(&env, root, "build.rs", "fn main() {}\n"),
    Decision::Allow,
    "no YAML, no spawn"
  );
}

#[test]
fn limits_come_from_lang_rust_and_the_exemption_list() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let settings = root.join("settings");
  std::fs::create_dir_all(root.join(".claude")).unwrap();
  std::fs::create_dir_all(&settings).unwrap();
  let config =
    json!({"lang": {"rust": {"maxFileLines": 300, "maxImplLines": 90, "noMocks": false}}});
  std::fs::write(root.join(".claude/toolu.config.json"), config.to_string()).unwrap();
  std::fs::write(settings.join("rust-unsafe-exemptions.txt"), "# why\nffi\n").unwrap();
  let env = cargo_repo(root).with("TOOLU_SETTINGS_DIR", &settings.display().to_string());
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let got = limits(&ctx);
  assert_eq!(
    (got.file_lines, got.fn_lines, got.impl_lines, got.no_mocks),
    (300, 50, 90, false)
  );
  assert_eq!(got.unsafe_exemptions, ["ffi"]);
  let list = settings.join("rust-unsafe-exemptions.txt");
  std::fs::set_permissions(&list, std::fs::Permissions::from_mode(0o000)).unwrap();
  assert!(
    limits(&ctx).unsafe_exemptions.is_empty(),
    "an unreadable list exempts nothing"
  );
  std::fs::set_permissions(&list, std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn an_unreadable_file_fails_its_check() {
  let (env, raw) = (Env::default(), Map::new());
  let root = Path::new("/nonexistent-root");
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: None,
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  let file = EditedFile {
    path: "src/gone.rs".into(),
    absolute: root.join("src/gone.rs"),
    removed: false,
  };
  let findings = RustQuality.check(&file, &ctx, &AstGrepScan::Missing);
  assert_eq!(findings.errors.len(), 1);
  assert!(findings.errors[0].starts_with("Cannot read src/gone.rs for Rust quality checks: "));
}
