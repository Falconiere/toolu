use super::{recap_lines, settle};
use crate::contract::{Coverage, Finding, Omitted, Severity, Side, Source, Verdict};
use crate::memory::{BranchState, Counts, Pass, PriorFinding, Status};
use crate::shape::{Rename, Shaped};
use crate::validate::Recap;

#[test]
fn recap_shows_ids_and_recent_passes_in_reverse_order() {
  let recap = Recap {
    resolved: vec!["fixed-id".into()],
    ..Recap::default()
  };
  let passes = vec![
    Pass {
      diff_sha: "old".into(),
      verdict: Verdict::Blocked,
      counts: Counts {
        blocking: 1,
        advisory: 0,
      },
    },
    Pass {
      diff_sha: "new".into(),
      verdict: Verdict::Approved,
      counts: Counts::default(),
    },
  ];
  let lines = recap_lines(&recap, &passes);
  assert_eq!(lines[0], "resolved: fixed-id");
  assert!(lines[1].contains("prior new: approved"));
  assert!(lines[2].contains("prior old: blocked"));
}

fn prior(id: &str, path: &str) -> PriorFinding {
  PriorFinding {
    fingerprint: String::new(),
    id: id.into(),
    path: path.into(),
    category: "correctness".into(),
    claim: "claim".into(),
    severity: Severity::High,
    status: Status::Open,
  }
}

fn shaped() -> Shaped {
  Shaped {
    base_ref: "main".into(),
    merge_base: String::new(),
    diff_sha: String::new(),
    gate_files: vec!["a.rs".into(), "new.rs".into()],
    files: Vec::new(),
    chunks: Vec::new(),
    coverage: Coverage::default(),
    renames: vec![Rename {
      old_path: "old.rs".into(),
      path: "new.rs".into(),
    }],
    uncommitted: false,
  }
}

#[test]
fn settle_keeps_reraised_and_renamed_priors_open_and_resolves_ones_out_of_the_diff() {
  let mut state = BranchState::default();
  state.memory.prior = vec![
    prior("gone", "removed.rs"),
    prior("kept", "a.rs"),
    prior("moved", "old.rs"),
    prior("prepass:secrets:a.rs:1:key", "a.rs"),
  ];
  let mut recap = Recap {
    resolved: vec!["prepass:secrets:a.rs:1:key".into()],
    still_open: vec!["gone".into(), "kept".into(), "moved".into()],
    ..Recap::default()
  };
  let raised = Finding {
    id: "prepass:secrets:a.rs:1:key".into(),
    path: "a.rs".into(),
    line: 1,
    end_line: 1,
    side: Side::New,
    severity: Severity::High,
    category: "key".into(),
    source: Source::Secrets,
    claim: "secret".into(),
    quote: "[REDACTED]".into(),
    suggestion: None,
    redaction_spans: Vec::new(),
    assessment: None,
  };
  settle(&mut recap, &[raised], &shaped(), &state);
  assert_eq!(recap.resolved, ["gone"]);
  assert_eq!(
    recap.still_open,
    ["kept", "moved", "prepass:secrets:a.rs:1:key"]
  );
}

#[test]
fn a_prior_on_a_now_skipped_path_does_not_block_forever() {
  let mut shaped = shaped();
  shaped.coverage.skipped.push(Omitted {
    path: "a.rs".into(),
    reason: "generated".into(),
  });
  let mut state = BranchState::default();
  state
    .memory
    .prior
    .push(prior("prepass:secrets:a.rs:1:key", "a.rs"));
  let mut recap = Recap {
    still_open: vec!["prepass:secrets:a.rs:1:key".into()],
    ..Recap::default()
  };
  settle(&mut recap, &[], &shaped, &state);
  assert_eq!(recap.still_open, Vec::<String>::new());
  assert_eq!(recap.resolved, ["prepass:secrets:a.rs:1:key"]);
}
