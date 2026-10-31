use super::packages;
use super::tests::LEVELS;
use super::{
  head,
  tests::{env, repo},
};
use crate::budget::Effort;
use crate::review::context::Callers;
use crate::review::mask::Mask;
use crate::shape::shape;

/// Run one `Callers` over every file of a feature that changes `RUN_FILES` files
/// each defining `run`, with `others` also in the repository; the contexts by
/// file, the notes and the `git grep` count.
fn shared_name(
  count: usize,
  others: &[(&str, String)],
) -> (Vec<Option<String>>, Vec<String>, usize) {
  let body = |tag: usize, value: u32| format!("pub fn run() -> u32 {{\n    {value} + {tag}\n}}\n");
  let names: Vec<String> = (0..count).map(|n| format!("src/m{n}.rs")).collect();
  let mut base: Vec<(&str, String)> = others.to_vec();
  base.extend(
    names
      .iter()
      .enumerate()
      .map(|(n, p)| (p.as_str(), body(n, 1))),
  );
  let change: Vec<(&str, String)> = names
    .iter()
    .enumerate()
    .map(|(n, p)| (p.as_str(), body(n, 2)))
    .collect();
  let dir = repo(&base, &change);
  let env = env();
  let shaped = shape(&env, dir.path(), Some("main")).unwrap();
  let mut callers = Callers::new(usize::MAX);
  let mut notes = Vec::new();
  let contexts = shaped
    .files
    .iter()
    .map(|file| {
      let text = head((&env, dir.path()), file, &mut notes).unwrap();
      callers.of((&env, dir.path()), &shaped, (file, &text), &mut notes)
    })
    .collect();
  (contexts, notes, callers.searches)
}

#[test]
fn a_name_enclosing_changes_in_several_files_is_searched_once() {
  let user = ("src/user.rs", "fn go() { run(); }\n".to_owned());
  let (contexts, notes, searches) = shared_name(3, &[user]);
  assert_eq!(searches, 1, "{notes:?}");
  // Each file's own definition line is excluded; the other files' are callers.
  for (index, context) in contexts.iter().enumerate() {
    let context = context.as_deref().unwrap_or_default();
    assert!(
      context.contains("src/user.rs:1: fn go() { run(); }\n"),
      "{context}"
    );
    assert!(
      !context.contains(&format!("src/m{index}.rs:1:")),
      "{context}"
    );
    let other = (index + 1) % 3;
    assert!(
      context.contains(&format!("src/m{other}.rs:1: pub fn run()")),
      "{context}"
    );
  }
}

#[test]
fn a_cached_over_limit_name_keeps_its_note_for_every_file() {
  let users: Vec<(String, String)> = (0..=crate::budget::CONTEXT_HITS_MAX)
    .map(|index| {
      (
        format!("src/u{index}.rs"),
        "fn go() { run(); }\n".to_owned(),
      )
    })
    .collect();
  let others: Vec<(&str, String)> = users.iter().map(|(p, t)| (p.as_str(), t.clone())).collect();
  let (contexts, notes, searches) = shared_name(2, &others);
  assert_eq!(searches, 1);
  assert!(contexts.iter().all(Option::is_none));
  let skipped = notes
    .iter()
    .filter(|n| n.starts_with("callers of run left out: over 20"))
    .count();
  assert_eq!(skipped, 2, "{notes:?}");
}

#[test]
fn a_name_with_no_other_reference_is_searched_once_and_sends_nothing() {
  let (contexts, notes, searches) = shared_name(2, &[]);
  // Only the two definitions name `run`; each file's is its own or changed.
  assert_eq!(searches, 1);
  assert!(
    contexts
      .iter()
      .all(|c| c.as_deref().is_none_or(|c| c.contains("src/m"))),
    "{notes:?}"
  );
}

#[test]
fn a_new_review_searches_the_new_head() {
  let (first, ..) = shared_name(1, &[("src/user.rs", "fn go() { run(); }\n".to_owned())]);
  let (second, ..) = shared_name(1, &[("src/other.rs", "fn go() { run(); }\n".to_owned())]);
  assert!(first[0].as_deref().unwrap().contains("src/user.rs"));
  let second = second[0].as_deref().unwrap();
  assert!(second.contains("src/other.rs") && !second.contains("src/user.rs"));
}

#[test]
fn an_uncommitted_reference_is_never_searched() {
  let changed = LEVELS.replace("_ => 2", "_ => 1");
  let dir = repo(
    &[("src/levels.rs", LEVELS.to_owned())],
    &[("src/levels.rs", changed)],
  );
  std::fs::write(
    dir.path().join("src/draft.rs"),
    "fn d() { parse_level(\"x\"); }\n",
  )
  .unwrap();
  let env = env();
  let shaped = shape(&env, dir.path(), Some("main")).unwrap();
  let budget = Effort::High.budget();
  let (found, notes) = packages(&env, dir.path(), &shaped, budget, &Mask::default());
  assert_eq!(found[0].context, None, "{notes:?}");
}
