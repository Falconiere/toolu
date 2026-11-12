use super::{Kind, State, grep_tool, message, shell, structural};

#[test]
fn nudge_real_shell_file_search_but_not_pipe_filter() {
  assert!(matches!(
    shell(&toolu_shell::analyze("grep -rn foo src/")),
    Some(Kind::BashGeneric)
  ));
  assert!(shell(&toolu_shell::analyze("ps aux | grep foo")).is_none());
  assert!(matches!(
    shell(&toolu_shell::analyze("grep -r 'impl Foo' src")),
    Some(Kind::BashStructural)
  ));
}

#[test]
fn nudge_structural_grep_only_for_code_targets() {
  let code = serde_json::json!({"pattern":"impl Foo for Bar", "glob":"*.rs"});
  let docs = serde_json::json!({"pattern":"impl Foo for Bar", "glob":"*.md"});
  let name = serde_json::json!({"pattern":"fn handle_request", "glob":"*.rs"});
  assert!(matches!(
    grep_tool(code.as_object().expect("object")),
    Some(Kind::GrepStructural)
  ));
  assert!(grep_tool(docs.as_object().expect("object")).is_none());
  assert!(grep_tool(name.as_object().expect("object")).is_none());
  assert!(!structural("fn handle_request"));
  assert!(!structural("pub fn run"));
  assert!(structural("impl Foo for Bar"));
  assert!(structural("-> Result"));
}

#[test]
fn nudge_points_declaration_greps_at_outline_and_symbol() {
  let kind = |command: &str| shell(&toolu_shell::analyze(command));
  assert!(matches!(
    kind(r#"grep -n "pub fn \|pub(crate) fn " crates/x/src/a.rs"#),
    Some(Kind::Outline)
  ));
  assert!(matches!(kind(r#"rg -n "^\s*(pub )?fn " src"#), Some(Kind::Outline)));
  assert!(matches!(kind(r#"rg "fn hunk_start""#), Some(Kind::Symbol)));
  assert!(matches!(kind(r#"rg "class Foo\(" src"#), Some(Kind::Symbol)));
  assert!(matches!(kind("rg TODO docs/"), Some(Kind::BashGeneric)));
  assert!(matches!(kind(r#"rg "fn x" docs/"#), Some(Kind::BashGeneric)));
  assert!(matches!(kind(r#"grep -rn "fn x" --include=*.md ."#), Some(Kind::BashGeneric)));
  assert!(matches!(kind(r#"grep -r "impl Foo" src"#), Some(Kind::BashStructural)));
  assert!(kind(r#"cat a.rs | grep "pub fn main""#).is_none());
}

#[test]
fn nudge_read_advisories_only_when_ast_grep_is_available() {
  let outline = message(Kind::Outline, State::Available).expect("outline text");
  assert!(outline.starts_with("Listing definitions? `toolu ast-grep outline <path>`"));
  let symbol = message(Kind::Symbol, State::Available).expect("symbol text");
  assert!(symbol.starts_with("Looking for a definition? `toolu ast-grep symbol <name>`"));
  for kind in [Kind::Outline, Kind::Symbol] {
    for state in [State::Missing, State::OptOut] {
      assert_eq!(message(kind, state), message(Kind::BashGeneric, state));
    }
  }
}
