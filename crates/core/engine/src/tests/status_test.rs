use std::path::Path;

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::LinkError;
use crate::status::StatusSnapshot;

/// A snapshot that reports the directory it was asked about.
struct Directory;

impl StatusSnapshot for Directory {
  fn snapshot(&self, _roots: &Roots, dir: &Path) -> Result<Value, LinkError> {
    if dir.as_os_str().is_empty() {
      return Err(LinkError::Failed("no directory".to_owned()));
    }
    Ok(json!({ "dir": dir.display().to_string() }))
  }
}

#[test]
fn a_snapshot_runs_through_a_trait_object() {
  let roots = Roots::new(Env::default(), None);
  let status: &dyn StatusSnapshot = &Directory;
  assert_eq!(
    status.snapshot(&roots, Path::new("/repo")),
    Ok(json!({ "dir": "/repo" }))
  );
  assert_eq!(
    status.snapshot(&roots, Path::new("")),
    Err(LinkError::Failed("no directory".to_owned()))
  );
}
