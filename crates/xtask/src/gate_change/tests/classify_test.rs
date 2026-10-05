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
