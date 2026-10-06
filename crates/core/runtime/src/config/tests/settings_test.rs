use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;

use super::{CodeEditRule, McpBlockEntry, code_edit_rules, mcp_blocklist, read_list, settings_dir};
use crate::env::Env;
use crate::host::roots::Roots;

fn shipped() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../plugins/toolu/settings")
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
  let path = dir.join(name);
  std::fs::write(&path, text).unwrap();
  path
}

#[test]
fn read_list_keeps_lines_verbatim_and_drops_comments_and_blanks() {
  let dir = tempfile::tempdir().unwrap();
  let text = "# comment\n   # indented comment\n\n \t \nvalue # not a comment\n  spaced  \nlast-without-newline";
  let path = write(dir.path(), "list.txt", text);
  assert_eq!(
    read_list(&path).unwrap(),
    [
      "value # not a comment",
      "  spaced  ",
      "last-without-newline"
    ]
  );
  assert_eq!(
    read_list(&dir.path().join("absent.txt")).unwrap(),
    Vec::<String>::new()
  );
  assert_eq!(
    read_list(&write(dir.path(), "empty.txt", "")).unwrap(),
    Vec::<String>::new()
  );
  assert_eq!(
    read_list(&write(dir.path(), "one.txt", "a\n")).unwrap(),
    ["a"]
  );
}

#[test]
fn the_shipped_lists_read() {
  let denylist = read_list(&shipped().join(super::BASH_DENYLIST)).unwrap();
  assert!(
    denylist.iter().any(|rule| rule == "cargo test"),
    "{denylist:?}"
  );
  assert_eq!(
    read_list(&shipped().join(super::BASH_ALLOWLIST)).unwrap(),
    Vec::<String>::new()
  );
  assert!(
    read_list(&shipped().join(super::COMMIT_PREFIXES))
      .unwrap()
      .contains(&"feat".to_owned())
  );
  assert!(
    read_list(&shipped().join(super::PROTECTED_FILES))
      .unwrap()
      .contains(&".env".to_owned())
  );
  assert_eq!(
    read_list(&shipped().join(super::RUST_UNSAFE_EXEMPTIONS)).unwrap(),
    Vec::<String>::new()
  );
  assert_eq!(
    mcp_blocklist(&shipped()).unwrap(),
    Vec::<McpBlockEntry>::new()
  );
}

#[test]
fn mcp_entries_split_as_mcp_blocker_did() {
  let cases = [
    (
      "claude_ai_Atlassian -> use the host's native Jira tools instead",
      "claude_ai_Atlassian",
      "use the host's native Jira tools instead",
    ),
    ("  figma  ", "figma", ""),
    ("canva -> a -> b", "canva", "a -> b"),
  ];
  for (line, prefix, redirect) in cases {
    let dir = tempfile::tempdir().unwrap();
    write(
      dir.path(),
      super::MCP_BLOCKLIST,
      &format!("# header\n{line}\n"),
    );
    let entry = McpBlockEntry {
      prefix: prefix.to_owned(),
      redirect: redirect.to_owned(),
    };
    assert_eq!(mcp_blocklist(dir.path()).unwrap(), [entry]);
  }
  let dir = tempfile::tempdir().unwrap();
  write(dir.path(), super::MCP_BLOCKLIST, "  -> hint only\n\tok \n");
  let ok = McpBlockEntry {
    prefix: "ok".to_owned(),
    redirect: String::new(),
  };
  assert_eq!(mcp_blocklist(dir.path()).unwrap(), [ok]);
}

#[test]
fn a_looping_link_reads_as_absent_and_bytes_decode_lossily() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir(dir.path().join(super::MCP_BLOCKLIST)).unwrap();
  let symlink = dir.path().join("loop.txt");
  std::os::unix::fs::symlink(&symlink, &symlink).unwrap();
  assert_eq!(read_list(&symlink).unwrap(), Vec::<String>::new());
  let lossy = dir.path().join("list.txt");
  std::fs::write(&lossy, [0xff, b'\n', b'a']).unwrap();
  assert_eq!(read_list(&lossy).unwrap(), ["\u{fffd}", "a"]);
}

#[test]
fn code_edit_rules_parse_the_shipped_file() {
  let rules = code_edit_rules(&shipped()).unwrap();
  let ts = rules.iter().find(|rule| rule.matches == "*.ts").unwrap();
  assert!(ts.when_path_matches.contains(&"*/components/*".to_owned()));
  assert_ne!(ts.extra_docs, Vec::<String>::new());
  let rust: Option<&CodeEditRule> = rules.iter().find(|rule| rule.matches == "*.rs");
  assert_eq!(rust.unwrap().when_path_matches, Vec::<String>::new());
}

#[test]
fn code_edit_rules_absent_is_empty_and_bad_files_are_errors() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(code_edit_rules(dir.path()), Ok(Vec::new()));
  write(dir.path(), super::CODE_EDIT_RULES, r#"{"rules": ["#);
  assert!(
    code_edit_rules(dir.path())
      .unwrap_err()
      .contains("code-edit-rules.json")
  );
  write(
    dir.path(),
    super::CODE_EDIT_RULES,
    r#"{"rules":[{"match":"*.ts","doc":[]}]}"#,
  );
  assert!(
    code_edit_rules(dir.path())
      .unwrap_err()
      .contains("code-edit-rules.json")
  );
}

#[test]
fn settings_dir_prefers_the_variable_then_the_legacy_dir_then_the_plugin_root() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path().display().to_string();
  let explicit = Roots::new(
    Env::from_pairs([("HOME", home.as_str()), ("TOOLU_SETTINGS_DIR", "/opt/s")]),
    Some(Host::Claude),
  );
  assert_eq!(settings_dir(&explicit, None), Some(PathBuf::from("/opt/s")));
  let plugin = Roots::new(
    Env::from_pairs([("HOME", home.as_str()), ("CLAUDE_PLUGIN_ROOT", "/p/toolu")]),
    Some(Host::Claude),
  );
  assert_eq!(
    settings_dir(&plugin, None),
    Some(PathBuf::from("/p/toolu/settings"))
  );
  assert_eq!(
    settings_dir(&plugin, Some(Path::new("/q"))),
    Some(PathBuf::from("/q/settings"))
  );
  let bare = Roots::new(
    Env::from_pairs([("HOME", home.as_str())]),
    Some(Host::Claude),
  );
  assert_eq!(settings_dir(&bare, None), None);
  std::fs::create_dir_all(dir.path().join(".claude/settings")).unwrap();
  assert_eq!(
    settings_dir(&plugin, None),
    Some(dir.path().join(".claude/settings"))
  );
}
