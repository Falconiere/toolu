use std::path::Path;

use super::{Change, classify};

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const DATA: &str = "tooling/conventions/guardrails/rust";

fn of(path: &str, before: Option<&str>, after: Option<&str>) -> Change {
  classify(Path::new(path), before, after, Path::new(REPO)).unwrap()
}

fn gate(path: &str, before: Option<&str>, after: Option<&str>) -> Option<String> {
  of(path, before, after).gate
}

#[test]
fn the_root_manifest_splits_lints_from_product() {
  let base = "[workspace]\nmembers = [\"a\"]\n\n[workspace.lints.clippy]\nall = \"deny\"\n";
  let lint = base.replace("\"deny\"", "\"warn\"");
  let member = base.replace("[\"a\"]", "[\"a\", \"b\"]");
  assert_eq!(
    of("Cargo.toml", Some(base), Some(&lint)),
    Change {
      gate: Some("Cargo.toml [workspace.lints] changed".to_owned()),
      product: false
    }
  );
  assert_eq!(
    of("Cargo.toml", Some(base), Some(&member)),
    Change {
      gate: None,
      product: true
    }
  );
}

#[test]
fn gate_files_configs_and_product_paths() {
  assert_eq!(
    gate("deny.toml", Some("a"), Some("b")).as_deref(),
    Some("deny.toml changed")
  );
  let config = "{\"comemory\":{},\"lang\":{\"rust\":{\"maxFileLines\":300}}}";
  let other = config.replace("\"comemory\":{}", "\"comemory\":{\"x\":1}");
  let raised = config.replace("300", "400");
  assert_eq!(
    gate(".claude/toolu.config.json", Some(config), Some(&other)),
    None
  );
  assert_eq!(
    gate(".codex/toolu.config.json", Some(config), Some(&raised)).as_deref(),
    Some(".codex/toolu.config.json lang.rust changed")
  );
  assert!(of("crates/a/src/lib.rs", None, Some("x")).product);
  for path in [
    "tooling/conventions/guardrails/web/oxlintrc.json",
    "tooling/conventions/guardrails/web/oxfmtrc.json",
    "tooling/conventions/guardrails/web/html-validate.json",
    "tooling/conventions/guardrails/web/jscpd.json",
    "tooling/conventions/guardrails/web/rules.json",
    "tooling/conventions/guardrails/web/sgconfig.yml",
    "tooling/conventions/guardrails/web/patterns/no-bare-fetch.yml",
    "crates/xtask/src/gate/web.rs",
    "crates/xtask/src/gate/web/checks.rs",
    "crates/xtask/src/gate.rs",
  ] {
    assert!(gate(path, None, Some("x")).is_some(), "{path}");
    assert!(!of(path, None, Some("x")).product, "{path}");
  }
  assert!(of("crates/xtask/src/pack.rs", None, Some("x")).product);
  assert!(of("Cargo.lock", Some("a"), Some("b")).product);
  assert_eq!(of("docs/x.md", Some("a"), Some("b")), Change::default());
}

#[test]
fn guardrail_data_is_gate_data_unless_it_only_grows() {
  assert!(gate(&format!("{DATA}/rules.json"), Some("{}"), Some("{\"a\":1}")).is_some());
  assert!(
    gate(&format!("{DATA}/new.json"), None, Some("{}"))
      .unwrap()
      .ends_with("is new gate data")
  );
  let folders = "{\"crates\":[\"a\"]}";
  assert_eq!(
    gate(
      &format!("{DATA}/folders.json"),
      Some(folders),
      Some("{\"crates\":[\"a\",\"b\"]}")
    ),
    None
  );
  assert!(
    gate(
      &format!("{DATA}/folders.json"),
      Some(folders),
      Some("{\"crates\":[\"b\"]}")
    )
    .unwrap()
    .contains("changed an existing entry")
  );
}

#[test]
fn coverage_floors_may_only_rise_and_start_at_the_default() {
  let path = format!("{DATA}/coverage-floor.json");
  let base = "{\"floors\":{\"demo\":90}}";
  assert_eq!(
    gate(
      &path,
      Some(base),
      Some("{\"floors\":{\"demo\":95,\"new\":85,\"toolu-shell\":90}}")
    ),
    None
  );
  assert_eq!(
    gate(&path, Some(base), Some("{\"floors\":{\"demo\":89}}")).as_deref(),
    Some("coverage floor of demo lowered to 89")
  );
  assert_eq!(
    gate(&path, Some(base), Some("{\"floors\":{}}")).as_deref(),
    Some("coverage floor of demo removed")
  );
  assert_eq!(
    gate(
      &path,
      Some(base),
      Some("{\"floors\":{\"demo\":90,\"new\":80}}")
    )
    .as_deref(),
    Some("coverage floor of new added at 80, below the default 85")
  );
  assert_eq!(
    gate(
      &path,
      Some(base),
      Some("{\"floors\":{\"demo\":90,\"toolu-shell\":86}}")
    )
    .as_deref(),
    Some("coverage floor of toolu-shell added at 86, below the default 90")
  );
}

#[test]
fn an_inventory_row_may_go_with_its_task_but_not_while_the_task_lives() {
  let path = format!("{DATA}/inventory.json");
  let row = |id: &str, pass: &str| {
    format!(r#"{{"kind":"xtask-task","id":"{id}","pass":"a.rs::{pass}","fail":"a.rs::f"}}"#)
  };
  let doc = |rows: &[String]| format!(r#"{{"entries":[{}]}}"#, rows.join(","));
  let gone = row("no-such-task", "p");
  let live = row("guardrails", "p");
  let before = doc(&[gone.clone(), live.clone()]);
  assert_eq!(gate(&path, Some(&before), Some(&doc(&[live]))), None);
  let dropped_live = gate(
    &path,
    Some(&before),
    Some(&doc(std::slice::from_ref(&gone))),
  );
  assert!(dropped_live.unwrap().contains("changed an existing entry"));
  let changed = gate(
    &path,
    Some(&before),
    Some(&doc(&[gone.clone(), row("guardrails", "q")])),
  );
  assert!(changed.unwrap().contains("changed an existing entry"));
  let unknown = r#"{"kind":"no-such-kind","id":"x","pass":"a.rs::p","fail":"a.rs::f"}"#.to_owned();
  let with_unknown = doc(&[unknown, gone.clone()]);
  let dropped_unknown = gate(&path, Some(&with_unknown), Some(&doc(&[gone])));
  assert!(
    dropped_unknown
      .unwrap()
      .contains("changed an existing entry")
  );
}

#[test]
fn an_unreadable_discovery_table_is_an_error() {
  let tmp = tempfile::tempdir().unwrap();
  let rules = format!("{DATA}/rules.json");
  std::fs::create_dir_all(tmp.path().join(DATA)).unwrap();
  std::fs::copy(Path::new(REPO).join(&rules), tmp.path().join(&rules)).unwrap();
  let row = r#"{"kind":"xtask-task","id":"gone","pass":"a.rs::p","fail":"a.rs::f"}"#;
  let before = format!(r#"{{"entries":[{row}]}}"#);
  let error = classify(
    Path::new(&format!("{DATA}/inventory.json")),
    Some(&before),
    Some(r#"{"entries":[]}"#),
    tmp.path(),
  )
  .unwrap_err();
  assert!(error.starts_with("crates/xtask/src/main.rs: "), "{error}");
}
