use crate::collect::{Jev, Project};
use crate::payload::read;

use super::line;

#[test]
fn empty_payload_has_exact_model_and_context_bytes() {
  let payload = read(Some("{}"));
  let project = Project {
    jev: Jev::Hidden,
    ..Project::default()
  };
  assert_eq!(
    line(&payload, &project),
    "\u{1b}[36mClaude\u{1b}[0m\u{1b}[2m | \u{1b}[0m\u{1b}[35mctx:0/0\u{1b}[0m"
  );
}

#[test]
fn half_even_percentage_and_dirty_order_match_existing_line() {
  let payload = read(Some(
    r#"{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":45000,"used_percentage":22.5}}"#,
  ));
  let project = Project {
    folder: "repo".to_owned(),
    repo_root: "/repo".to_owned(),
    branch: "main".to_owned(),
    ahead: 2,
    staged: 1,
    unstaged: 1,
    untracked: 3,
    jev: Jev::Hidden,
    ..Project::default()
  };
  let output = line(&payload, &project);
  assert!(output.contains("ctx:45k/200k (22%)"));
  assert!(output.contains("main\u{1b}[0m\u{1b}[2m↑2\u{1b}[0m\u{1b}[33m[+1 ~1 ?3]"));
}
