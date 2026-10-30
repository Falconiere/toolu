use super::{Kind, grep_tool, shell, structural};

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
