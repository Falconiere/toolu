use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;

use super::{PublishOptions, PublishResult, Published, publish};
use crate::env::Env;
use crate::host::roots::Roots;
use crate::startup::report::HelperStatus;

struct Fixture {
  dir: tempfile::TempDir,
  roots: Roots,
  source: PathBuf,
}

/// A plugin source file, a config root and a startup report file.
fn fixture() -> Fixture {
  let dir = tempfile::tempdir().unwrap();
  let source = dir.path().join("plugin/jev.sh");
  std::fs::create_dir_all(source.parent().unwrap()).unwrap();
  std::fs::write(&source, "#!/bin/sh\n").unwrap();
  let config = dir.path().join("config").display().to_string();
  let report = dir.path().join("report.jsonl").display().to_string();
  let env = Env::from_pairs([
    ("TOOLU_CONFIG_DIR", config),
    ("TOOLU_STARTUP_REPORT", report),
  ]);
  Fixture {
    dir,
    roots: Roots::new(env, Some(Host::Claude)),
    source,
  }
}

impl Fixture {
  fn path(&self) -> PathBuf {
    self.dir.path().join("config/jev/jev.sh")
  }

  fn publish(&self, source: &Path) -> Published {
    let options = PublishOptions {
      plugin: "jev",
      source,
      dir: "jev",
      name: "jev.sh",
      what: "wrapper",
      roots: &self.roots,
    };
    publish(&options)
  }

  fn report(&self) -> String {
    std::fs::read_to_string(self.dir.path().join("report.jsonl")).unwrap_or_default()
  }

  fn line(&self, status: &str) -> String {
    let (path, source) = (
      self.path().display().to_string(),
      self.source.display().to_string(),
    );
    format!(
      r#"{{"kind":"helper","plugin":"jev","source":"{source}","path":"{path}","status":"{status}"}}"#
    ) + "\n"
  }
}

fn outcome(status: HelperStatus, path: Option<PathBuf>) -> Published {
  Published {
    result: PublishResult { status, path },
    warning: None,
    report_error: None,
  }
}

#[test]
fn a_users_regular_file_is_never_overwritten() {
  let f = fixture();
  std::fs::create_dir_all(f.path().parent().unwrap()).unwrap();
  std::fs::write(f.path(), "my own wrapper\n").unwrap();
  assert_eq!(
    f.publish(&f.source),
    outcome(HelperStatus::KeptUserFile, Some(f.path()))
  );
  let meta = std::fs::symlink_metadata(f.path()).unwrap();
  assert!(meta.file_type().is_file());
  assert_eq!(
    std::fs::read_to_string(f.path()).unwrap(),
    "my own wrapper\n"
  );
  assert_eq!(f.report(), f.line("kept-user-file"));
}

#[test]
fn a_directory_at_the_path_is_kept_too() {
  let f = fixture();
  std::fs::create_dir_all(f.path()).unwrap();
  assert_eq!(
    f.publish(&f.source).result.status,
    HelperStatus::KeptUserFile
  );
  assert!(f.path().is_dir());
}

#[test]
fn a_stale_or_dangling_link_is_replaced_and_a_correct_one_kept() {
  let f = fixture();
  std::fs::create_dir_all(f.path().parent().unwrap()).unwrap();
  std::os::unix::fs::symlink(f.dir.path().join("gone.sh"), f.path()).unwrap();
  assert_eq!(
    f.publish(&f.source),
    outcome(HelperStatus::Published, Some(f.path()))
  );
  assert_eq!(std::fs::read_link(f.path()).unwrap(), f.source);
  assert_eq!(f.publish(&f.source).result.status, HelperStatus::Published);
  assert_eq!(f.report(), f.line("published").repeat(2));
  let leftovers = std::fs::read_dir(f.path().parent().unwrap())
    .unwrap()
    .count();
  assert_eq!(leftovers, 1, "no temp link left behind");
}

#[test]
fn a_missing_source_publishes_nothing_and_reports_no_path() {
  let f = fixture();
  let missing = f.dir.path().join("plugin/absent.sh");
  assert_eq!(
    f.publish(&missing),
    outcome(HelperStatus::SourceMissing, None)
  );
  assert!(!f.path().exists());
  let line = format!(
    r#"{{"kind":"helper","plugin":"jev","source":"{}","status":"source-missing"}}"#,
    missing.display()
  );
  assert_eq!(f.report(), line + "\n");
}

#[test]
fn an_uncreatable_directory_warns_and_a_broken_report_is_returned() {
  let f = fixture();
  std::fs::write(
    f.dir.path().join("config"),
    "a file where the directory goes",
  )
  .unwrap();
  let published = f.publish(&f.source);
  let dir = f.dir.path().join("config/jev");
  assert_eq!(
    published.result,
    PublishResult {
      status: HelperStatus::Unwritable,
      path: Some(dir.clone())
    }
  );
  assert_eq!(
    published.warning,
    Some(format!(
      "jev: cannot create {} — wrapper not published",
      dir.display()
    ))
  );
  assert_eq!(published.report_error, None);
  let env = f.roots.env().clone().with(
    "TOOLU_STARTUP_REPORT",
    &f.dir.path().join("config/r.jsonl").display().to_string(),
  );
  let roots = Roots::new(env, Some(Host::Claude));
  let options = PublishOptions {
    plugin: "jev",
    source: &f.source,
    dir: "jev",
    name: "jev.sh",
    what: "helper",
    roots: &roots,
  };
  let error = publish(&options).report_error.unwrap();
  assert!(
    error.starts_with("toolu-startup: cannot write startup report "),
    "{error}"
  );
}
