//! Per-step scope hashes on a real repository.

use crate::ledger::context::test_repo::Repo;
use toolu_runtime::json::ordered::Ordered;

use super::{scope_map, scope_sha};
use crate::ledger::jq::parse_json;

#[test]
fn a_scope_hash_changes_with_its_paths_only() {
  let repo = Repo::new().unwrap();
  let env = repo.opts().env().clone();
  let docs = vec!["docs".to_owned()];
  let empty = scope_sha("main", &docs, &repo.root, &env).unwrap();
  repo.sh("echo y >> a.ts && git commit -qam code").unwrap();
  assert_eq!(
    scope_sha("main", &docs, &repo.root, &env).as_deref(),
    Some(empty.as_str())
  );
  repo
    .sh("mkdir docs && echo d > docs/x.md && git add docs && git commit -qm docs")
    .unwrap();
  let changed = scope_sha("main", &docs, &repo.root, &env).unwrap();
  assert_ne!(changed, empty);
  let other = scope_sha(
    "main",
    &["docs".to_owned(), "a.ts".to_owned()],
    &repo.root,
    &env,
  );
  assert!(other.is_some_and(|sha| sha != changed));
  assert_eq!(scope_sha("main", &[], &repo.root, &env), None);
  assert_eq!(scope_sha("no-such-base", &docs, &repo.root, &env), None);
}

#[test]
fn the_scope_map_skips_unscoped_steps_and_warns_on_unhashable_ones() {
  let repo = Repo::new().unwrap();
  let env = repo.opts().env().clone();
  let steps: Vec<Ordered> = [
    r#"{"id":"s1","paths":["a.ts"]}"#,
    r#"{"id":"s2","paths":[]}"#,
    r#"{"id":"","paths":["a.ts"]}"#,
    r#"{"id":"s4"}"#,
  ]
  .iter()
  .map(|text| parse_json(text).unwrap())
  .collect();
  let mut warnings = Vec::new();
  let map = scope_map(&steps, "main", &repo.root, &env, &mut |line| {
    warnings.push(line);
  })
  .unwrap();
  let Ordered::Object(entries) = &map else {
    panic!("not an object");
  };
  assert_eq!(
    entries
      .iter()
      .map(|(id, _)| id.as_str())
      .collect::<Vec<_>>(),
    ["s1"]
  );
  assert_eq!(warnings, Vec::<String>::new());
  let broken = scope_map(&steps, "no-such-base", &repo.root, &env, &mut |line| {
    warnings.push(line);
  })
  .unwrap();
  assert_eq!(broken, Ordered::Object(Vec::new()));
  assert_eq!(
    warnings,
    [
      "plan-ledger: step s1 declares paths that could not be hashed; judging it on the whole branch diff"
    ]
  );
  assert!(
    scope_map(
      &[parse_json("3").unwrap()],
      "main",
      &repo.root,
      &env,
      &mut |_| {}
    )
    .is_err()
  );
}
