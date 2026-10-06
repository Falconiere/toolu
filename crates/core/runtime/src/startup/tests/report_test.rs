use super::{HelperStatus, RegistryStatus, StartupRecord, report};
use crate::env::Env;
use crate::registry::RegistryEvent;

#[test]
fn every_record_is_one_json_line_in_typescript_key_order() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("report.jsonl");
  let env = Env::from_pairs([("TOOLU_STARTUP_REPORT", file.display().to_string())]);
  let records = [
    StartupRecord::Registry {
      spec: "ts-quality@toolu".to_owned(),
      name: "max-lines".to_owned(),
      event: RegistryEvent::ToolPost,
      source: "/p/a.js".to_owned(),
      target: "/c/post-tools.d/a.js".to_owned(),
      status: RegistryStatus::Failed,
      error: Some("EACCES".to_owned()),
    },
    StartupRecord::Registry {
      spec: "s".to_owned(),
      name: "n".to_owned(),
      event: RegistryEvent::ToolPre,
      source: "a".to_owned(),
      target: "b".to_owned(),
      status: RegistryStatus::Unchanged,
      error: None,
    },
    StartupRecord::Helper {
      plugin: "jev".to_owned(),
      source: "/p/jev.sh".to_owned(),
      path: None,
      status: HelperStatus::SourceMissing,
    },
    StartupRecord::Error {
      origin: "prune".to_owned(),
      message: "stale".to_owned(),
    },
  ];
  for record in &records {
    report(&env, record).unwrap();
  }
  let expected = [
    r#"{"kind":"registry","spec":"ts-quality@toolu","name":"max-lines","event":"tool/post","source":"/p/a.js","target":"/c/post-tools.d/a.js","status":"failed","error":"EACCES"}"#,
    r#"{"kind":"registry","spec":"s","name":"n","event":"tool/pre","source":"a","target":"b","status":"unchanged"}"#,
    r#"{"kind":"helper","plugin":"jev","source":"/p/jev.sh","status":"source-missing"}"#,
    r#"{"kind":"error","origin":"prune","message":"stale"}"#,
  ];
  assert_eq!(
    std::fs::read_to_string(&file).unwrap(),
    expected.join("\n") + "\n"
  );
}

#[test]
fn without_a_report_file_nothing_is_written_and_a_bad_path_is_an_error() {
  let record = StartupRecord::Error {
    origin: "o".to_owned(),
    message: "m".to_owned(),
  };
  assert_eq!(
    report(&Env::from_pairs([("TOOLU_STARTUP_REPORT", "")]), &record),
    Ok(())
  );
  let dir = tempfile::tempdir().unwrap();
  let missing = dir
    .path()
    .join("no/such/dir/report.jsonl")
    .display()
    .to_string();
  let error = report(
    &Env::from_pairs([("TOOLU_STARTUP_REPORT", missing.as_str())]),
    &record,
  )
  .unwrap_err();
  assert!(
    error.starts_with(&format!(
      "toolu-startup: cannot write startup report {missing}: "
    )),
    "{error}"
  );
}
