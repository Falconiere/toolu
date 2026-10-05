use std::path::Path;

use super::{Manifest, read};

fn plugin(claude: Option<&str>, codex: Option<&str>) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  for (sub, text) in [(".claude-plugin", claude), (".codex-plugin", codex)] {
    if let Some(text) = text {
      std::fs::create_dir_all(dir.path().join(sub)).unwrap();
      std::fs::write(dir.path().join(sub).join("plugin.json"), text).unwrap();
    }
  }
  dir
}

#[test]
fn the_real_toolu_manifest_reads() {
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../plugins/toolu");
  let manifest = read(&root).unwrap();
  assert_eq!(manifest.hook_protocol, 1);
  assert_eq!(manifest.version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn claude_comes_first_and_codex_is_the_fallback() {
  let both = plugin(
    Some(r#"{"version":"1.0.0","hookProtocol":1}"#),
    Some(r#"{"version":"2.0.0","hookProtocol":1}"#),
  );
  assert_eq!(read(both.path()).unwrap().version, "1.0.0");
  let codex = plugin(None, Some(r#"{"version":"2.0.0","hookProtocol":3}"#));
  assert_eq!(
    read(codex.path()).unwrap(),
    Manifest {
      version: "2.0.0".to_owned(),
      hook_protocol: 3
    }
  );
}

#[test]
fn a_bad_claude_manifest_does_not_fall_back() {
  let dir = plugin(
    Some("{not json"),
    Some(r#"{"version":"2.0.0","hookProtocol":1}"#),
  );
  assert!(read(dir.path()).unwrap_err().contains("is not JSON"));
}

#[test]
fn hook_protocol_must_be_an_integer_from_1_to_u32_max() {
  for value in ["0", "-1", "\"1\"", "1.0", "4294967296", "null"] {
    let dir = plugin(
      Some(&format!(r#"{{"version":"1.0.0","hookProtocol":{value}}}"#)),
      None,
    );
    assert!(
      read(dir.path()).unwrap_err().contains("hookProtocol"),
      "{value}"
    );
  }
  let absent = plugin(Some(r#"{"version":"1.0.0"}"#), None);
  assert!(read(absent.path()).is_err());
  let max = plugin(Some(r#"{"hookProtocol":4294967295}"#), None);
  assert_eq!(read(max.path()).unwrap().hook_protocol, u32::MAX);
  assert_eq!(read(max.path()).unwrap().version, "");
}

#[test]
fn a_missing_manifest_or_empty_root_is_an_error() {
  let empty = plugin(None, None);
  assert!(read(empty.path()).unwrap_err().contains("cannot read"));
  assert_eq!(read(Path::new("")).unwrap_err(), "the plugin root is empty");
}
