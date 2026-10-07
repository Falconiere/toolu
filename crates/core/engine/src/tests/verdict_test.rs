//! The verdict report, its table and its `status|json` output on a real repository.

use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;

use super::{render_verdict_status, verdict_main, verdict_report};
use crate::ledger::context::test_repo::Repo;
use crate::ledger::io::LedgerOptions;
use crate::ledger::jq::parse_json;

#[test]
fn a_fresh_branch_is_blocked_and_its_table_names_every_gate() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let (report, warnings) = verdict_report(&opts).unwrap();
  assert_eq!(warnings, Vec::<String>::new());
  assert_eq!(
    report.get("overall"),
    Some(&Ordered::String("blocked".to_owned()))
  );
  assert_eq!(
    report.get("generated_at"),
    Some(&Ordered::String("2031-02-03T04:05:06Z".to_owned()))
  );
  let status = verdict_main(false, &opts);
  assert_eq!(status.exit, 1);
  let sha = match report.get("diff_sha") {
    Some(Ordered::String(sha)) => sha.chars().take(12).collect::<String>(),
    _ => String::new(),
  };
  assert_eq!(
    status.stdout,
    format!(
      "verdict: feat/x vs main (diff {sha})\nGATE     STATE     REASON\nquality  pass      no quality-gate failure recorded\nplan     advise    no plan ledger for this change; if non-trivial, run plan\nreview   fail      no push-review state file; run a reviewer and write the state\ndocs     advise    code changed without a doc update\noverall: blocked\n"
    )
  );
  let json = verdict_main(true, &opts);
  assert_eq!(
    json.stdout,
    format!("{}\n", jq_text(json.ledger.as_ref().unwrap(), true))
  );
  repo.sh("true").unwrap();
}

#[test]
fn outside_a_repository_or_without_git_the_verdict_is_an_error() {
  let repo = Repo::new().unwrap();
  let outside = LedgerOptions {
    cwd: std::env::temp_dir(),
    ..repo.opts()
  };
  let result = verdict_main(true, &outside);
  assert_eq!(
    (result.exit, result.stderr.as_str()),
    (2, "verdict: not a git repository\n")
  );
  let env = toolu_runtime::env::Env::from_pairs([("PATH", "/nonexistent")]);
  let no_git = LedgerOptions {
    roots: Roots::new(env, None),
    ..repo.opts()
  };
  assert_eq!(
    verdict_report(&no_git).map(|_| ()),
    Err("verdict: git is required".to_owned())
  );
}

#[test]
fn the_table_pads_like_printf_and_cuts_the_hash_to_twelve() {
  let report = parse_json(
    r#"{"branch":"b","base_branch":"m","diff_sha":"0123456789abcdef","overall":"ready","gates":{"quality":{"state":"pass","reason":"q"},"plan":{"state":"skip","reason":"p"},"review":{"state":"pass","reason":"r"},"docs":{"state":"pass","reason":"d"}}}"#,
  )
  .unwrap();
  assert_eq!(
    render_verdict_status(&report),
    "verdict: b vs m (diff 0123456789ab)\nGATE     STATE     REASON\nquality  pass      q\nplan     skip      p\nreview   pass      r\ndocs     pass      d\noverall: ready\n"
  );
}
