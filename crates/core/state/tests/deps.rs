//! The state layer carries neither a shell parser nor TLS (AC-7): the normal
//! dependency tree of `toolu-state`, as `cargo tree` prints it, holds none of
//! the crates `rules.json` reserves for `toolu-shell` or `toolu-http`.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

type Res<T> = Result<T, String>;

fn repo() -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The crates `rules.json` reserves for the shell parser and HTTP owners.
fn reserved() -> Res<BTreeSet<String>> {
  let file = repo().join("tooling/conventions/guardrails/rust/rules.json");
  let text = std::fs::read_to_string(file).map_err(|err| err.to_string())?;
  let rules: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
  let groups = rules
    .get("capabilityCrates")
    .and_then(Value::as_array)
    .ok_or("no capabilityCrates")?;
  let owned = groups
    .iter()
    .filter(|group| {
      matches!(
        group.get("id").and_then(Value::as_str),
        Some("http" | "shell-parser")
      )
    })
    .flat_map(|group| {
      group
        .get("crates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    });
  Ok(
    owned
      .filter_map(|name| name.as_str().map(str::to_owned))
      .collect(),
  )
}

/// Every crate in the normal dependency tree of `toolu-state`.
fn tree() -> Res<BTreeSet<String>> {
  let out = Command::new(env!("CARGO"))
    .args([
      "tree",
      "-p",
      "toolu-state",
      "-e",
      "normal",
      "--prefix",
      "none",
      "--format",
      "{p}",
      "--offline",
    ])
    .current_dir(repo())
    .output()
    .map_err(|err| err.to_string())?;
  if !out.status.success() {
    return Err(String::from_utf8_lossy(&out.stderr).into_owned());
  }
  let text = String::from_utf8_lossy(&out.stdout).into_owned();
  Ok(
    text
      .lines()
      .filter_map(|line| line.split_whitespace().next())
      .map(str::to_owned)
      .collect(),
  )
}

#[test]
fn no_shell_parser_or_tls_reaches_the_state_crate() {
  let (reserved, tree) = (reserved().unwrap(), tree().unwrap());
  assert!(
    reserved.contains("rustls") && reserved.contains("brush-parser"),
    "{reserved:?}"
  );
  assert!(
    tree.contains("toolu-runtime") && tree.contains("tempfile"),
    "{tree:?}"
  );
  let reached: Vec<&String> = tree.intersection(&reserved).collect();
  assert_eq!(reached, Vec::<&String>::new());
}
