//! Replays every case of `fixtures/gates/lifecycle.json` against the captures in
//! `fixtures/gates/lifecycle-golden.json` with the built `toolu hook` command, as the
//! retired TypeScript suites did through the `TOOLU_IMPL` seam: `SessionStart` compares its
//! context and the first line of its title, `UserPromptSubmit` every byte.

#[path = "helpers/golden_lifecycle_cases.rs"]
mod golden_lifecycle_cases;
#[path = "helpers/golden_lifecycle_compare.rs"]
mod golden_lifecycle_compare;
#[path = "helpers/golden_lifecycle_run.rs"]
mod golden_lifecycle_run;
#[path = "helpers/golden_lifecycle_sandbox.rs"]
mod golden_lifecycle_sandbox;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use golden_lifecycle_cases::{Case, Golden, Res, read_cases, read_golden, repo_root};
use golden_lifecycle_compare::{diff, session_parts};
use golden_lifecycle_run::run_case;

/// Cases whose golden depends on behaviour that no longer exists: name and reason.
const EXCLUDED: &[(&str, &str)] = &[];

const SESSION_START_CASES: usize = 50;
const USER_PROMPT_SUBMIT_CASES: usize = 67;
const WORKERS: usize = 8;

/// `None` when `case` matches its capture, else a description of the mismatch.
fn check(case: &Case, golden: &Golden, plugin: &Path) -> Res<Option<String>> {
  let want = golden.cases.get(&case.name).ok_or("no golden capture")?;
  let got = run_case(case, plugin)?;
  diff(case, &got, want)
}

/// One worker: take the next case until none is left, noting each mismatch by name.
fn work(cases: &[&Case], next: &AtomicUsize, golden: &Golden, plugin: &Path) -> Vec<String> {
  let mut failures = Vec::new();
  while let Some(case) = cases.get(next.fetch_add(1, Ordering::SeqCst)) {
    match check(case, golden, plugin) {
      Ok(None) => {}
      Ok(Some(diff)) => failures.push(format!("--- {}\n{diff}", case.name)),
      Err(err) => failures.push(format!("--- {}\nerror: {err}", case.name)),
    }
  }
  failures
}

fn replay(cases: &[&Case], golden: &Golden, plugin: &Path) -> Vec<String> {
  let next = AtomicUsize::new(0);
  let all = thread::scope(|scope| {
    let workers: Vec<_> = (0..WORKERS)
      .map(|_| scope.spawn(|| work(cases, &next, golden, plugin)))
      .collect();
    workers
      .into_iter()
      .map(|worker| {
        worker
          .join()
          .unwrap_or_else(|_| vec!["a worker panicked".to_owned()])
      })
      .collect::<Vec<_>>()
  });
  all.into_iter().flatten().collect()
}

#[test]
fn every_lifecycle_case_matches_its_golden_capture() {
  let root = repo_root().unwrap();
  let plugin = root.join("plugins/toolu");
  let cases = read_cases(&root).unwrap();
  let golden = read_golden(&root, &cases).unwrap();
  assert_eq!(golden.base, "107e566f");
  let count = |hook: &str| cases.iter().filter(|case| case.hook == hook).count();
  assert_eq!(count("session-start"), SESSION_START_CASES);
  assert_eq!(count("user-prompt-submit"), USER_PROMPT_SUBMIT_CASES);
  assert_eq!(cases.len(), golden.cases.len());
  let excluded: Vec<&str> = EXCLUDED.iter().map(|(name, _)| *name).collect();
  let live: Vec<&Case> = cases
    .iter()
    .filter(|case| !excluded.contains(&case.name.as_str()))
    .collect();
  assert_eq!(
    live.len() + EXCLUDED.len(),
    cases.len(),
    "an excluded name is not a case"
  );
  let replayed = live.len();
  assert_eq!(
    replayed,
    SESSION_START_CASES + USER_PROMPT_SUBMIT_CASES - EXCLUDED.len()
  );
  let failures = replay(&live, &golden, &plugin);
  assert!(
    failures.is_empty(),
    "{} of {replayed} cases differ:\n{}",
    failures.len(),
    failures.join("\n")
  );
}

#[test]
fn the_excluded_list_is_empty_or_every_entry_has_a_reason() {
  assert!(EXCLUDED.iter().all(|(_, reason)| !reason.is_empty()));
}

#[test]
fn research_wrappers_and_exa_key_add_no_research_mandate() {
  let root = repo_root().unwrap();
  let cases = read_cases(&root).unwrap();
  let case = cases
    .iter()
    .find(|case| case.name.contains("deprecated research wrappers"))
    .unwrap();
  let got = run_case(case, &root.join("plugins/toolu")).unwrap();
  let context = session_parts(&got.stdout)
    .unwrap()
    .context
    .unwrap_or_default();
  assert!(
    context.contains("ast-grep (structural search)"),
    "{context}"
  );
  assert_eq!(context.matches("\n  • ").count(), 2, "{context}");
}

/// The context of a `UserPromptSubmit` run on `prompt` in a plain sandbox.
fn prompt_context(name: &str, prompt: &str) -> Res<String> {
  let root = repo_root()?;
  let case = Case {
    name: name.to_owned(),
    hook: "user-prompt-submit".to_owned(),
    stdin: serde_json::json!({ "prompt": prompt }).to_string(),
    ..Case::default()
  };
  let got = run_case(&case, &root.join("plugins/toolu"))?;
  if got.exit_code != 0 {
    return Err(format!("exit {}: {got:?}", got.exit_code).into());
  }
  let out: serde_json::Value = serde_json::from_str(&got.stdout)?;
  let context = out.pointer("/hookSpecificOutput/additionalContext");
  let context = context.and_then(serde_json::Value::as_str);
  Ok(context.ok_or("no additionalContext")?.to_owned())
}

#[test]
fn latest_release_research_uses_the_agent_without_third_party_tools() {
  let context = prompt_context("native latest release", "what is the latest Bun release").unwrap();
  assert!(context.contains("research-agent"), "{context}");
  assert!(context.contains("native web"), "{context}");
  assert!(!context.contains("search.sh"), "{context}");
}

#[test]
fn an_atlassian_browse_link_adds_no_jira_hint() {
  let prompt = "see https://acme.atlassian.net/browse/ABC-123";
  let context = prompt_context("native Atlassian link", prompt).unwrap();
  assert!(!context.contains("Jira mentioned"), "{context}");
}
