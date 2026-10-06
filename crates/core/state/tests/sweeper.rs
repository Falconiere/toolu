//! The sweeper on real repositories (AC-10): per-branch state of merged, gone
//! and stale branches goes, the current branch's and a fresh live branch's
//! stay; a passing gate file and one failing only on missing files go, one
//! failing on a live file or `__global__` stays, an unrecognized one stays;
//! telemetry is trimmed to the retention window; `gates.sweep: false` stops it.

#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sandbox::{Res, Sandbox};
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_schema::GLOBAL_GATE_KEY;
use toolu_state::sweeper::{
  SWEEP_BRANCH_DIRS, SWEEP_DEFAULT_RETENTION_DAYS, SWEEP_DEFAULT_TTL_HOURS, sweep_state,
};
use toolu_state::time::iso_seconds;

/// A repository on `feat/current` with `feat/merged` (merged into `main`),
/// and unmerged `feat/ahead` and `feat/old`; its state root and config.
fn repo(config: &str) -> Res<(Sandbox, PathBuf)> {
  let sb = Sandbox::new(Some("main"))?;
  sb.git(&["branch", "feat/merged"])?;
  for branch in ["feat/ahead", "feat/old", "feat/current"] {
    sb.git(&["checkout", "-q", "-b", branch, "main"])?;
    sb.git(&["commit", "-q", "--allow-empty", "-m", branch])?;
  }
  let state = sb.project.join(".claude/tmp");
  std::fs::create_dir_all(&state).map_err(|err| err.to_string())?;
  std::fs::write(sb.project.join(".claude/toolu.config.json"), config)
    .map_err(|err| err.to_string())?;
  Ok((sb, state))
}

/// Writes `body` at `path`, last modified `age` ago.
fn put(path: &Path, body: &str, age: Duration) -> Res<()> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?).map_err(|err| err.to_string())?;
  std::fs::write(path, body).map_err(|err| err.to_string())?;
  let file = File::options()
    .write(true)
    .open(path)
    .map_err(|err| err.to_string())?;
  file
    .set_modified(SystemTime::now() - age)
    .map_err(|err| err.to_string())
}

fn sweep(sb: &Sandbox) -> Vec<String> {
  let mut ctx = StateCtx::new(Roots::new(sb.env(), Some(Host::Claude)));
  sweep_state(&mut ctx, Some(&sb.project));
  ctx.warnings
}

const FRESH: Duration = Duration::from_secs(60);
/// Past the default TTL.
const OLD: Duration = Duration::from_hours(SWEEP_DEFAULT_TTL_HOURS * 2);

#[test]
fn branch_state_of_spent_branches_is_reclaimed() {
  let (sb, state) = repo("{\"version\":1}").unwrap();
  let files = [
    ("push-review/feat_current.json", OLD, true),
    ("push-review/feat_merged.json", FRESH, false),
    ("push-review/feat_gone.json", FRESH, false),
    ("push-review/feat_ahead.json", FRESH, true),
    ("push-review/feat_old.json", OLD, false),
    ("plan-ledger/feat_ahead.waiver.json", FRESH, true),
    ("docs-sync/feat_merged.pending-waiver.json", FRESH, false),
    ("docs-sync/.feat_gone.json", FRESH, true),
    ("docs-sync/feat_gone.txt", FRESH, true),
  ];
  for (name, age, _) in files {
    put(&state.join(name), "{}", age).unwrap();
  }
  assert_eq!(sweep(&sb), Vec::<String>::new());
  for (name, _, kept) in files {
    assert_eq!(state.join(name).exists(), kept, "{name}");
  }
}

#[test]
fn every_branch_state_dir_is_swept_and_telemetry_keeps_its_default_window() {
  let (sb, state) = repo("{\"version\":1}").unwrap();
  for dir in SWEEP_BRANCH_DIRS {
    put(&state.join(dir).join("feat_gone.json"), "{}", FRESH).unwrap();
  }
  let days = |n: u64| iso_seconds(SystemTime::now() - Duration::from_hours(24 * n));
  let (inside, outside) = (
    days(SWEEP_DEFAULT_RETENTION_DAYS - 1),
    days(SWEEP_DEFAULT_RETENTION_DAYS + 1),
  );
  let lines = format!("{{\"t\":\"{outside}\"}}\n{{\"t\":\"{inside}\"}}\n");
  put(&state.join("telemetry/feat_x.jsonl"), &lines, FRESH).unwrap();
  sweep(&sb);
  for dir in SWEEP_BRANCH_DIRS {
    assert!(!state.join(dir).join("feat_gone.json").exists(), "{dir}");
  }
  let kept = std::fs::read_to_string(state.join("telemetry/feat_x.jsonl")).unwrap();
  assert_eq!(kept, format!("{{\"t\":\"{inside}\"}}\n"));
}

#[test]
fn a_short_ttl_reclaims_a_live_branch_sooner() {
  let (sb, state) = repo("{\"version\":1,\"gates\":{\"stateTtlHours\":1}}").unwrap();
  put(
    &state.join("push-review/feat_ahead.json"),
    "{}",
    Duration::from_hours(2),
  )
  .unwrap();
  sweep(&sb);
  assert!(!state.join("push-review/feat_ahead.json").exists());
}

#[test]
fn gate_files_go_only_when_spent() {
  let failing = |keys: &str| {
    format!(
      "{{\"status\":\"failing\",\"reason\":\"r\",\"source\":\"s\",\"file\":\"x\",\"violations\":\"v\",\"entries\":{{{keys}}},\"updatedAt\":\"u\"}}"
    )
  };
  let entry = |key: &str| {
    format!(
      "\"{key}\":{{\"source\":\"s\",\"reason\":\"r\",\"violations\":\"v\",\"updatedAt\":\"u\"}}"
    )
  };
  let live_file = std::env::temp_dir().display().to_string();
  let cases = [
    ("{\"status\":\"passing\",\"source\":\"s\",\"updatedAt\":\"u\"}".to_owned(), false),
    (failing(&entry("/nowhere/a.ts")), false),
    (failing(&format!("{},{}", entry("/nowhere/a.ts"), entry(&live_file))), true),
    (failing(&entry(GLOBAL_GATE_KEY)), true),
    ("{\"status\":\"failing\",\"reason\":\"r\",\"source\":\"s\",\"file\":\"/nowhere\",\"violations\":\"v\",\"updatedAt\":\"u\"}".to_owned(), false),
    ("{\"status\":\"passing\",\"source\":\"s\",\"updatedAt\":\"u\",\"owner\":\"x\"}".to_owned(), true),
  ];
  for (doc, kept) in cases {
    let (sb, state) = repo("{\"version\":1}").unwrap();
    let gate = state.join("quality-gate-status.json");
    put(&gate, &doc, OLD).unwrap();
    assert_eq!(sweep(&sb), Vec::<String>::new());
    assert_eq!(gate.exists(), kept, "{doc}");
    assert!(!state.join("quality-gate-status.json.lock").exists());
  }
}

#[test]
fn telemetry_is_trimmed_to_the_retention_window() {
  let (sb, state) = repo("{\"version\":1,\"gates\":{\"telemetryRetentionDays\":2}}").unwrap();
  let dir = state.join("telemetry");
  let at = |ago: Duration| iso_seconds(SystemTime::now() - ago);
  let (old, new) = (at(Duration::from_hours(72)), at(Duration::from_hours(1)));
  let mixed = format!("{{\"t\":\"{old}\",\"e\":1}}\n{{\"t\":\"{new}\",\"e\":2}}\n");
  put(&dir.join("feat_x.jsonl"), &mixed, FRESH).unwrap();
  put(
    &dir.join("feat_y.jsonl"),
    &format!("{{\"t\":\"{old}\"}}\n"),
    FRESH,
  )
  .unwrap();
  let broken = format!("{{\"t\":\"{old}\"}}\nnot json\n");
  put(&dir.join("feat_z.jsonl"), &broken, FRESH).unwrap();
  assert_eq!(sweep(&sb), Vec::<String>::new());
  let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
  assert_eq!(
    read("feat_x.jsonl"),
    Some(format!("{{\"t\":\"{new}\",\"e\":2}}\n"))
  );
  assert_eq!(read("feat_y.jsonl"), None);
  assert_eq!(read("feat_z.jsonl"), Some(broken));
}

#[test]
fn a_disabled_sweep_or_a_missing_state_root_touches_nothing() {
  let (sb, state) = repo("{\"version\":1,\"gates\":{\"sweep\":false}}").unwrap();
  put(&state.join("push-review/feat_gone.json"), "{}", OLD).unwrap();
  sweep(&sb);
  assert!(state.join("push-review/feat_gone.json").exists());
  let (bare, state) = repo("{\"version\":1}").unwrap();
  std::fs::remove_dir_all(&state).unwrap();
  assert_eq!(sweep(&bare), Vec::<String>::new());
}

#[test]
fn symlinked_state_is_never_followed_out_of_the_repository() {
  let (sb, state) = repo("{\"version\":1}").unwrap();
  let victim = sb.home.join("victim");
  put(&victim.join("feat_gone.json"), "{}", OLD).unwrap();
  put(
    &victim.join("feat_x.jsonl"),
    "{\"t\":\"2000-01-01T00:00:00Z\"}\n",
    OLD,
  )
  .unwrap();
  std::os::unix::fs::symlink(&victim, state.join("push-review")).unwrap();
  std::os::unix::fs::symlink(&victim, state.join("telemetry")).unwrap();
  std::fs::create_dir_all(state.join("plan-ledger")).unwrap();
  std::os::unix::fs::symlink(
    victim.join("feat_gone.json"),
    state.join("plan-ledger/feat_gone.json"),
  )
  .unwrap();
  assert_eq!(sweep(&sb), Vec::<String>::new());
  assert!(victim.join("feat_gone.json").exists() && victim.join("feat_x.jsonl").exists());
  assert!(
    state.join("plan-ledger/feat_gone.json").exists(),
    "the link itself is not judged"
  );
  let (other, tmp) = repo("{\"version\":1}").unwrap();
  std::fs::remove_dir_all(&tmp).unwrap();
  let elsewhere = sb.home.join("elsewhere");
  put(&elsewhere.join("push-review/feat_gone.json"), "{}", OLD).unwrap();
  std::os::unix::fs::symlink(&elsewhere, &tmp).unwrap();
  assert_eq!(sweep(&other), Vec::<String>::new());
  assert!(
    elsewhere.join("push-review/feat_gone.json").exists(),
    "a symlinked tmp dir is not swept"
  );
}

#[test]
fn branch_state_is_kept_when_git_cannot_list_the_branches() {
  let (sb, state) = repo("{\"version\":1}").unwrap();
  put(&state.join("push-review/feat_gone.json"), "{}", OLD).unwrap();
  let env = sb.env().with("GIT_DIR", "/nonexistent");
  let mut ctx = StateCtx::new(Roots::new(env, Some(Host::Claude)));
  sweep_state(&mut ctx, Some(&sb.project));
  assert!(state.join("push-review/feat_gone.json").exists());
  let warning = format!(
    "toolu-sweep: cannot list the branches of {}; branch state kept",
    sb.project.display()
  );
  assert_eq!(ctx.warnings, [warning]);
}

#[test]
fn telemetry_is_decoded_lossily_and_an_untrimmable_file_warns() {
  let (sb, state) = repo("{\"version\":1}").unwrap();
  let dir = state.join("telemetry");
  let new = iso_seconds(SystemTime::now());
  let mut bytes = format!("{{\"t\":\"{new}\",\"n\":\"").into_bytes();
  bytes.extend_from_slice(b"\xff\"}\n{\"t\":\"2000-01-01T00:00:00Z\"}\n");
  put(&dir.join("feat_x.jsonl"), "", FRESH).unwrap();
  std::fs::write(dir.join("feat_x.jsonl"), bytes).unwrap();
  let long = dir.join(format!("{}.jsonl", "q".repeat(245)));
  put(
    &long,
    &format!("{{\"t\":\"{new}\"}}\n{{\"t\":\"2000-01-01T00:00:00Z\"}}\n"),
    FRESH,
  )
  .unwrap();
  let warnings = sweep(&sb);
  let kept = std::fs::read_to_string(dir.join("feat_x.jsonl")).unwrap();
  assert_eq!(kept, format!("{{\"t\":\"{new}\",\"n\":\"\u{fffd}\"}}\n"));
  assert_eq!(
    warnings,
    [format!("toolu-sweep: could not trim {}", long.display())]
  );
}
