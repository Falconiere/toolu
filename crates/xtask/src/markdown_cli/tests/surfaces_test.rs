use serde_json::Value;

use super::{bun_script, patterns, problem, references};
use crate::markdown_cli::shell::lex;

fn tree() -> Value {
  let text = std::fs::read_to_string(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/cli/commands.json"
  ))
  .unwrap();
  serde_json::from_str(&text).unwrap()
}

/// The real tree with `namespace`'s placeholder verb turned into a real `run`.
fn ported(namespace: &str) -> Value {
  let mut tree = tree();
  let commands = tree["commands"].as_array_mut().unwrap();
  let command = commands
    .iter_mut()
    .find(|command| command["name"] == namespace)
    .unwrap();
  let verbs = command["commands"].as_array_mut().unwrap();
  for verb in verbs.iter_mut().filter(|verb| verb["placeholder"] == true) {
    verb["placeholder"] = Value::Bool(false);
    verb["name"] = Value::from("run");
  }
  tree
}

#[test]
fn paths_of_removed_surfaces_are_found_with_their_stems() {
  let patterns = patterns().unwrap();
  let found = references(
    &patterns,
    r#"bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run x; "$TOOLU_BUN" --no-env-file scripts/epic-graph.ts 402; "$X/jev/jev.sh" ask"#,
  );
  assert_eq!(
    found,
    [
      (
        "hooks/dist/plan-ledger.js".to_owned(),
        "plan-ledger".to_owned()
      ),
      ("scripts/epic-graph.ts".to_owned(), "epic-graph".to_owned()),
      ("jev.sh".to_owned(), "jev".to_owned()),
    ]
  );
  assert_eq!(
    references(
      &patterns,
      "\"$R/toolu-review/write-state.sh\" --x; x=jev.sh"
    ),
    [
      ("write-state.sh".to_owned(), "write-state".to_owned()),
      ("jev.sh".to_owned(), "jev".to_owned()),
    ]
  );
  assert_eq!(
    references(
      &patterns,
      "comemory.sh save x; my-jev.sh; ast-grep-search.sh; bun test packages/a.test.ts"
    ),
    []
  );
}

#[test]
fn bun_running_a_script_is_a_reference_unless_a_path_pattern_caught_it() {
  let patterns = patterns().unwrap();
  let first = |text: &str| lex(text, 1).commands.into_iter().next().unwrap();
  assert_eq!(
    bun_script(&patterns, &first("bun \"$S/launch-issue.ts\" --graph g")),
    Some(("$S/launch-issue.ts".to_owned(), "launch-issue".to_owned()))
  );
  assert_eq!(
    bun_script(
      &patterns,
      &first("\"$TOOLU_BUN\" --no-env-file ./x/route.js")
    ),
    Some(("./x/route.js".to_owned(), "route".to_owned()))
  );
  assert_eq!(
    bun_script(&patterns, &first("bun run ./launch-issue.ts --dry-run")),
    Some(("./launch-issue.ts".to_owned(), "launch-issue".to_owned()))
  );
  assert_eq!(
    bun_script(&patterns, &first("bun --cwd dir route.ts")),
    Some(("route.ts".to_owned(), "route".to_owned()))
  );
  for text in [
    "bun test packages/x.test.ts",
    "bun build src/x.ts --outdir dist",
    "bun run test",
    "bun \"$R/hooks/dist/verdict.js\" status",
    "gh x.ts",
    "bun",
  ] {
    assert_eq!(bun_script(&patterns, &first(text)), None, "{text}");
  }
}

#[test]
fn a_reference_fails_only_once_its_namespace_is_ported() {
  let real = tree();
  assert_eq!(problem(&real, "plan-ledger", "delivery-flow"), None);
  let message = problem(&ported("ledger"), "plan-ledger", "delivery-flow").unwrap();
  assert!(message.contains("`toolu ledger` is ported"), "{message}");
  assert_eq!(
    problem(&ported("ledger"), "babysit-tick", "pr-babysit"),
    None
  );
  assert!(problem(&ported("babysit"), "babysit-tick", "pr-babysit").is_some());
}

#[test]
fn unmapped_stems_fall_back_to_the_plugins_own_namespaces() {
  assert_eq!(problem(&tree(), "launch-issue", "epic-orchestrator"), None);
  assert!(problem(&ported("epic"), "launch-issue", "epic-orchestrator").is_some());
  // brainstorm owns a namespace with no placeholder verb: already ported.
  assert!(problem(&tree(), "anything", "brainstorm").is_some());
  // a plugin owning several namespaces needs all of them ported.
  assert_eq!(problem(&ported("ledger"), "anything", "toolu"), None);
  // stems match exactly unless the entry is a `-` prefix.
  assert!(problem(&ported("statusline"), "status", "toolu").is_some());
  assert_eq!(
    problem(&ported("statusline"), "status-board", "toolu"),
    None
  );
  assert!(problem(&ported("babysit"), "babysit-route-fix", "toolu").is_some());
  // an unknown plugin owns nothing.
  assert_eq!(problem(&tree(), "anything", "nope"), None);
}
