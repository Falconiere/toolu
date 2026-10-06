//! The `io` cases of `fixtures/state/cases.json` (AC-4): jq text checked
//! against the real `jq`, byte-order sorting, timestamps, atomic writes and
//! every lock scenario, one test per scenario, as `state-io.test.ts` runs
//! them for TypeScript.

#[path = "helpers/cases.rs"]
mod cases;

use std::fs::File;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use cases::{cases_of, field, text};
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::io::write_atomic;
use toolu_state::lock::{LockOptions, lock_path, with_lock};
use toolu_state::time::{iso_seconds, parse_iso};

type Res<T> = Result<T, String>;

const SCENARIOS: [&str; 13] = [
  "jq-json",
  "jq-sort",
  "iso",
  "atomic-replace",
  "atomic-missing",
  "lock-result",
  "stale-lock",
  "live-lock",
  "atomic-mode",
  "dead-pid-lock",
  "stale-own-pid",
  "lock-takeover",
  "stale-no-broken",
];

/// The cases of `scenario`, each with a fresh directory and its `file` there.
fn scenario(scenario: &str) -> Res<Vec<(Ordered, tempfile::TempDir, PathBuf)>> {
  let all = cases_of("state/cases.json", "io")?;
  let wanted = Ordered::String(scenario.to_owned());
  let mut out = Vec::new();
  for case in all
    .into_iter()
    .filter(|case| case.get("scenario") == Some(&wanted))
  {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let file = dir
      .path()
      .join(text(&case, "file").unwrap_or_else(|_| "gate.json".to_owned()));
    out.push((case, dir, file));
  }
  Ok(out)
}

fn number(case: &Ordered, key: &str) -> Res<u64> {
  if let Ordered::Number(number) = field(case, key)? {
    return number
      .as_u64()
      .ok_or_else(|| format!("{key} is not a whole number"));
  }
  Err(format!("{key} is not a number"))
}

fn words(value: &Ordered) -> Vec<String> {
  let Ordered::Array(items) = value else {
    return Vec::new();
  };
  let strings = items.iter().map(|item| {
    if let Ordered::String(word) = item {
      Some(word.clone())
    } else {
      None
    }
  });
  strings.flatten().collect()
}

/// `jq <args>` over `input`; its stdout. A missing `jq` fails the test.
fn jq(args: &[String], input: &str) -> Res<String> {
  let mut child = Command::new("jq")
    .args(args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .map_err(|err| format!("jq is required: {err}"))?;
  child
    .stdin
    .take()
    .ok_or("no stdin")?
    .write_all(input.as_bytes())
    .map_err(|err| err.to_string())?;
  let out = child.wait_with_output().map_err(|err| err.to_string())?;
  String::from_utf8(out.stdout).map_err(|err| err.to_string())
}

fn age(lock: &Path, by: Duration) -> Res<()> {
  let file = File::options()
    .write(true)
    .open(lock)
    .map_err(|err| err.to_string())?;
  file
    .set_modified(SystemTime::now() - by)
    .map_err(|err| err.to_string())
}

fn lock_names(dir: &Path) -> Vec<String> {
  let names = std::fs::read_dir(dir).into_iter().flatten().flatten();
  names
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| name.contains(".lock"))
    .collect()
}

#[test]
fn the_fourteen_cases_cover_thirteen_scenarios() {
  let all = cases_of("state/cases.json", "io").unwrap();
  assert_eq!(all.len(), 14);
  for case in &all {
    assert!(SCENARIOS.contains(&text(case, "scenario").unwrap().as_str()));
  }
}

#[test]
fn jq_json_is_byte_for_byte_what_jq_prints() {
  for (case, _, _) in scenario("jq-json").unwrap() {
    let args = words(field(&case, "jqArgs").unwrap());
    let value = field(&case, "value").unwrap();
    let pretty = field(&case, "pretty").unwrap() == &Ordered::Bool(true);
    assert_eq!(
      format!("{}\n", jq_text(value, pretty)),
      jq(&args, &value.to_text(false)).unwrap()
    );
  }
}

#[test]
fn byte_order_sorts_as_jq_sorts_and_not_as_javascript_does() {
  for (case, _, _) in scenario("jq-sort").unwrap() {
    let mut ours = words(field(&case, "words").unwrap());
    let input = Ordered::Array(ours.iter().cloned().map(Ordered::String).collect()).to_text(false);
    let jqs = jq(&words(field(&case, "jqArgs").unwrap()), &input).unwrap();
    ours.sort();
    assert_eq!(words(&Ordered::parse(&jqs).unwrap()), ours);
    let mut utf16 = ours.clone();
    utf16.sort_by_key(|word| word.encode_utf16().collect::<Vec<u16>>());
    assert_ne!(utf16, ours);
  }
}

#[test]
fn iso_seconds_truncate() {
  for (case, _, _) in scenario("iso").unwrap() {
    let t = parse_iso(&text(&case, "input").unwrap()).unwrap();
    assert_eq!(iso_seconds(t), text(&case, "expected").unwrap());
  }
}

#[test]
fn atomic_writes_replace_refuse_and_create_0600() {
  for (case, dir, file) in scenario("atomic-replace").unwrap() {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, text(&case, "before").unwrap()).unwrap();
    assert!(write_atomic(&file, &text(&case, "after").unwrap()));
    assert_eq!(
      std::fs::read_to_string(&file).unwrap(),
      text(&case, "after").unwrap()
    );
    let names: Vec<String> = std::fs::read_dir(dir.path().join("state"))
      .unwrap()
      .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
      .collect();
    assert_eq!(names, words(field(&case, "remaining").unwrap()));
  }
  for (case, _, file) in scenario("atomic-missing").unwrap() {
    assert!(!write_atomic(&file, &text(&case, "body").unwrap()));
    assert!(!file.exists());
  }
  for (case, _, file) in scenario("atomic-mode").unwrap() {
    assert!(write_atomic(&file, &text(&case, "body").unwrap()));
    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(u64::from(mode), number(&case, "mode").unwrap());
  }
}

#[test]
fn the_lock_is_held_during_and_released_after_even_a_panic() {
  for (_, _, file) in scenario("lock-result").unwrap() {
    let lock = lock_path(&file);
    assert!(with_lock(
      &file,
      LockOptions::default(),
      &mut Vec::new(),
      || lock.exists()
    ));
    assert!(!lock.exists());
    let panicked = std::panic::catch_unwind(|| {
      with_lock(&file, LockOptions::default(), &mut Vec::new(), || {
        panic!("boom")
      })
    });
    assert!(panicked.is_err());
    assert!(!lock.exists());
  }
}

#[test]
fn a_dead_holders_lock_is_broken_at_once() {
  for (case, _, file) in scenario("dead-pid-lock").unwrap() {
    let shell = Command::new("bash")
      .args(["-c", "echo $$"])
      .output()
      .unwrap();
    let pid = String::from_utf8_lossy(&shell.stdout).trim().to_owned();
    std::fs::write(
      lock_path(&file),
      format!("{pid}{}", text(&case, "lockSuffix").unwrap()),
    )
    .unwrap();
    let (mut warnings, started) = (Vec::new(), Instant::now());
    assert!(with_lock(
      &file,
      LockOptions::default(),
      &mut warnings,
      || true
    ));
    assert!(started.elapsed() < Duration::from_millis(number(&case, "maxMs").unwrap()));
    assert_eq!(warnings, Vec::<String>::new());
  }
}

#[test]
fn a_live_holders_lock_is_broken_once_stale() {
  for (case, _, file) in scenario("stale-own-pid").unwrap() {
    let content = format!(
      "{}{}",
      std::process::id(),
      text(&case, "lockSuffix").unwrap()
    );
    std::fs::write(lock_path(&file), content).unwrap();
    age(
      &lock_path(&file),
      Duration::from_millis(number(&case, "ageMs").unwrap()),
    )
    .unwrap();
    let mut warnings = Vec::new();
    assert!(with_lock(
      &file,
      LockOptions::default(),
      &mut warnings,
      || true
    ));
    assert_eq!(warnings, Vec::<String>::new());
    assert!(!lock_path(&file).exists());
  }
}

#[test]
fn a_lock_taken_over_is_left_to_its_new_holder() {
  for (case, _, file) in scenario("lock-takeover").unwrap() {
    let other = text(&case, "otherLock").unwrap();
    with_lock(&file, LockOptions::default(), &mut Vec::new(), || {
      std::fs::write(lock_path(&file), &other)
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(lock_path(&file)).unwrap(), other);
  }
}

#[test]
fn a_crashed_writers_stale_lock_is_broken_without_litter() {
  for (case, dir, file) in scenario("stale-lock")
    .unwrap()
    .into_iter()
    .chain(scenario("stale-no-broken").unwrap())
  {
    std::fs::write(lock_path(&file), text(&case, "lock").unwrap()).unwrap();
    age(
      &lock_path(&file),
      Duration::from_millis(number(&case, "ageMs").unwrap()),
    )
    .unwrap();
    let mut warnings = Vec::new();
    assert!(with_lock(
      &file,
      LockOptions::default(),
      &mut warnings,
      || true
    ));
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(lock_names(dir.path()), Vec::<String>::new());
  }
}

#[test]
fn a_live_lock_times_out_warns_and_runs_unlocked() {
  for (case, dir, file) in scenario("live-lock").unwrap() {
    std::fs::write(lock_path(&file), text(&case, "lock").unwrap()).unwrap();
    let timeout = Duration::from_millis(number(&case, "timeoutMs").unwrap());
    let options = LockOptions {
      timeout,
      ..LockOptions::default()
    };
    let (mut warnings, started) = (Vec::new(), Instant::now());
    assert!(with_lock(&file, options, &mut warnings, || true));
    assert!(started.elapsed() >= timeout);
    let project = dir.path().display().to_string();
    let warning = text(field(&case, "warning").unwrap(), "$template")
      .unwrap()
      .replace("$PROJECT", &project);
    assert_eq!(warnings, [warning]);
    assert!(lock_path(&file).exists());
  }
}
