use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::edit_records::{EditOperation, EditRecord};

use super::Batch;
use crate::dispatch::edits::synthetic_edit;
use crate::dispatch::session::Session;
use crate::dispatch::{DEFAULT_MODULE_TIMEOUT, DispatchOptions, Phase};

fn capture_module(dir: &Path, log: &Path) {
  let module = dir.join("probe@toolu__capture.sh");
  std::fs::write(
    &module,
    format!(
      "#!/usr/bin/env bash\nIFS= read -r line\ntest \"$line\" = \"$input\" || exit 2\nprintf '%s\\n' \"$line\" >> '{}'\n",
      log.display()
    ),
  )
  .unwrap();
  std::fs::set_permissions(&module, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn records() -> [EditRecord; 2] {
  ["first.rs", "second.rs"].map(|path| EditRecord {
    path: path.to_owned(),
    operation: EditOperation::Write,
    from: None,
    moved_to: None,
  })
}

fn edit_doc() -> Ordered {
  Ordered::parse(
    r#"{"tool_name":"apply_patch","tool_input":{"command":"patch text","path":"old"},"session_id":"s"}"#,
  )
  .unwrap()
}

#[test]
fn each_reached_executable_gets_its_exact_synthetic_payload() {
  let temp = tempfile::tempdir().unwrap();
  let project = temp.path().join("project");
  let config = temp.path().join("home/.claude");
  let dir = config.join("toolu/pre-tools.d");
  std::fs::create_dir_all(&project).unwrap();
  std::fs::create_dir_all(&dir).unwrap();
  let log = temp.path().join("inputs.jsonl");
  capture_module(&dir, &log);
  let env = Env::process().with("HOME", &temp.path().join("home").to_string_lossy());
  let options = DispatchOptions {
    env: &env,
    cwd: &project,
    lib_dir: Path::new("/lib"),
    builtins: &[],
    rules: &[],
    quality_rules: &[],
    selected_specs: None,
    continue_post_blocks: false,
    split_by_caller: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  };
  let session = Session {
    phase: Phase::Pre,
    host: Host::Claude,
    env: env.clone(),
    roots: Roots::new(env.clone(), Some(Host::Claude)),
    config_root: config,
    project_root: project.clone(),
    options: &options,
  };
  let doc = edit_doc();
  let records = records();
  let mut batch = Batch::new(&doc, &records[0], &records, &session).unwrap();
  let mut trace = Vec::new();
  for (index, record) in records.iter().enumerate() {
    let result = batch.walk_path((&doc, record, index), &session, &mut trace);
    assert_eq!(result.exit_code, 0, "{result:?}");
  }
  let captured = std::fs::read_to_string(log).unwrap();
  let mut expected = String::new();
  for record in &records {
    expected.push_str(&jq_text(&synthetic_edit(&doc, record), false));
    expected.push('\n');
  }
  assert_eq!(captured, expected);
  assert_eq!(trace.len(), 2);
}
