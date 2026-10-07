//! The quality gate and the helpers every verdict gate shares.

use toolu_runtime::json::ordered::Ordered;

use super::{field_or, gate, quality_gate, raw_or, read_json};
use crate::ledger::context::test_repo::Repo;
use crate::ledger::jq::{get, parse_json, string};

fn reason(gate: &Ordered) -> (String, String) {
  let field = |key| match gate.get(key) {
    Some(Ordered::String(text)) => text.clone(),
    _ => String::new(),
  };
  (field("state"), field("reason"))
}

#[test]
fn the_quality_gate_reads_the_gate_file() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let ctx = repo.gate(&opts.roots, "feat/x", "");
  let pair = |state: &str, text: &str| (state.to_owned(), text.to_owned());
  assert_eq!(
    reason(&quality_gate(&ctx)),
    pair("pass", "no quality-gate failure recorded")
  );
  let file = repo.root.join(".claude/tmp/quality-gate-status.json");
  repo.sh("mkdir -p .claude/tmp").unwrap();
  std::fs::write(&file, r#"{"status":"passing"}"#).unwrap();
  assert_eq!(
    reason(&quality_gate(&ctx)),
    pair("pass", "quality gate passing")
  );
  std::fs::write(
    &file,
    "{\"status\":\"failing\",\"reason\":\"lint failed\\n\\n\"}",
  )
  .unwrap();
  assert_eq!(reason(&quality_gate(&ctx)), pair("fail", "lint failed"));
  std::fs::write(&file, r#"{"status":"failing","reason":null}"#).unwrap();
  assert_eq!(
    reason(&quality_gate(&ctx)),
    pair("fail", "Quality gate failing")
  );
  std::fs::write(&file, "{oops").unwrap();
  assert_eq!(
    reason(&quality_gate(&ctx)),
    pair("pass", "quality gate passing")
  );
}

#[test]
fn a_linked_worktree_skips_the_quality_gate() {
  let repo = Repo::new().unwrap();
  repo.sh("git worktree add -q ../linked -b feat/y").unwrap();
  let opts = repo.opts();
  let mut linked = repo.gate(&opts.roots, "feat/x", "");
  linked.root = repo.root.join("../linked");
  assert_eq!(
    reason(&quality_gate(&linked)).1,
    "gate disabled in linked worktree"
  );
}

#[test]
fn helpers_read_json_like_jq() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let ctx = repo.gate(&opts.roots, "feat/x", "");
  assert_eq!(
    ctx.git(&["rev-parse", "--abbrev-ref", "HEAD"]).as_deref(),
    Some("feat/x\n")
  );
  assert_eq!(ctx.git(&["no-such-command"]), None);
  assert_eq!(
    ctx.state_dir("push-review", "STATE_DIR"),
    repo.root.join(".claude/tmp/push-review")
  );
  let doc = parse_json(r#"{"a":"x","b":false,"c":3}"#).unwrap();
  assert_eq!(
    (
      field_or(&doc, "a", "d"),
      field_or(&doc, "b", "d"),
      field_or(&doc, "c", "d")
    ),
    ("x".to_owned(), "d".to_owned(), "3".to_owned())
  );
  assert_eq!(field_or(&string("s"), "a", "fallback"), "fallback");
  assert_eq!(raw_or(get(&string("s"), "a"), "f"), "f");
  assert_eq!(read_json(&repo.root.join("absent.json")), None);
  let made = gate("skip", "why", vec![("x".to_owned(), Ordered::Null)]);
  assert_eq!(
    made.to_text(false),
    r#"{"state":"skip","reason":"why","x":null}"#
  );
}
