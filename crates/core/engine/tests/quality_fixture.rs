//! Shared TypeScript quality-runner JSON over a real Git project.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value, json};
use toolu_engine::quality::{EditedFile, edited_file, in_linked_worktree, is_regular_file};
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::{EditOperation, EditSplit, RuleContext};

type Res<T> = Result<T, String>;

struct Project {
  _temp: tempfile::TempDir,
  root: PathBuf,
  repo: PathBuf,
  home: PathBuf,
}

impl Project {
  fn new() -> Res<Self> {
    let temp = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = std::fs::canonicalize(temp.path()).map_err(|err| err.to_string())?;
    let project = root.join("project");
    let home = root.join("home");
    std::fs::create_dir_all(&project).map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&home).map_err(|err| err.to_string())?;
    let out = Command::new("git")
      .args(["init", "-q"])
      .current_dir(&project)
      .output()
      .map_err(|err| err.to_string())?;
    if !out.status.success() {
      return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let out = Command::new("git")
      .args([
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "init",
      ])
      .current_dir(&project)
      .output()
      .map_err(|err| err.to_string())?;
    if !out.status.success() {
      return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(Self {
      _temp: temp,
      root,
      repo: project,
      home,
    })
  }

  fn text(&self, input: &str) -> String {
    input
      .replace("$PROJECT", &self.repo.to_string_lossy())
      .replace("$ROOT", &self.root.to_string_lossy())
      .replace("$HOME", &self.home.to_string_lossy())
  }

  fn value(&self, value: &Value) -> Value {
    match value {
      Value::String(text) => Value::String(self.text(text)),
      Value::Object(map) if map.len() == 1 && map.contains_key("$path") => map
        .get("$path")
        .map_or(Value::Null, |value| self.value(value)),
      Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
        value.clone()
      }
    }
  }

  fn setup(&self, case: &Value) -> Res<()> {
    let Some(steps) = case.get("setup").and_then(Value::as_array) else {
      return Ok(());
    };
    for step in steps {
      match step.get("op").and_then(Value::as_str) {
        Some("write") => self.write_step(step)?,
        Some("git") => self.git_step(step)?,
        _ => return Err("unknown setup operation".to_owned()),
      }
    }
    Ok(())
  }

  fn write_step(&self, step: &Value) -> Res<()> {
    let path = self.text(step["path"].as_str().ok_or("write path")?);
    let path = Path::new(&path);
    std::fs::create_dir_all(path.parent().ok_or("write parent")?).map_err(|err| err.to_string())?;
    std::fs::write(path, step["body"].as_str().ok_or("write body")?).map_err(|err| err.to_string())
  }

  fn git_step(&self, step: &Value) -> Res<()> {
    let args = step["args"].as_array().ok_or("git args")?;
    let args: Vec<String> = args
      .iter()
      .map(|arg| self.value(arg).as_str().unwrap_or_default().to_owned())
      .collect();
    let out = Command::new("git")
      .args(&args)
      .current_dir(&self.repo)
      .output()
      .map_err(|err| err.to_string())?;
    if out.status.success() {
      Ok(())
    } else {
      Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
  }
}

fn operation(name: &str) -> EditOperation {
  match name {
    "delete" => EditOperation::Delete,
    "move" => EditOperation::Move,
    "add" => EditOperation::Add,
    "write" => EditOperation::Write,
    _ => EditOperation::Update,
  }
}

fn file_for_call(project: &Project, call: &Value) -> Res<Option<EditedFile>> {
  let input = call["input"].as_object().cloned().unwrap_or_default();
  let tool = call["toolName"].as_str().unwrap_or("Write");
  let raw: Map<String, Value> = [
    ("tool_name".to_owned(), Value::String(tool.to_owned())),
    ("tool_input".to_owned(), Value::Object(input.clone())),
  ]
  .into_iter()
  .collect();
  let mut env = Env::from_pairs([
    ("HOME", project.home.to_string_lossy().into_owned()),
    ("PATH", "/usr/bin:/bin".to_owned()),
  ]);
  if let Some(fields) = call["env"].as_object() {
    for (key, value) in fields {
      env = env.with(key, value.as_str().ok_or("environment value")?);
    }
  }
  let split = call["edit"].as_object().map(|edit| EditSplit {
    operation: operation(
      edit
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("update"),
    ),
    from: edit.get("from").and_then(Value::as_str).unwrap_or(""),
    moved_to: edit.get("movedTo").and_then(Value::as_str).unwrap_or(""),
  });
  let ctx = RuleContext {
    host: toolu_protocol::host::Host::Claude,
    env: &env,
    config_root: &project.home,
    project_root: &project.repo,
    cwd: Some(&project.repo),
    plugin_root: None,
    raw: &raw,
    edit: split,
  };
  let event: NormalizedEvent = serde_json::from_value(json!({
    "type": "tool/post", "sessionId": "s", "cwd": project.repo,
    "projectRoot": project.repo, "worktree": project.repo,
    "toolCallId": "t", "toolName": tool, "toolInput": input
  }))
  .map_err(|err| err.to_string())?;
  Ok(edited_file(&event, &ctx))
}

fn observed(project: &Project, file: Option<&EditedFile>, field: &str) -> Option<Value> {
  match field {
    "path" => file.map(|file| Value::String(file.path.clone())),
    "removed" => file.map(|file| Value::Bool(file.removed)),
    "full" => file.map(|file| {
      let absolute = file
        .absolute
        .to_string_lossy()
        .replace(project.repo.to_string_lossy().as_ref(), "<P>");
      json!({"path": file.path, "absolute": absolute, "removed": file.removed})
    }),
    _ => None,
  }
}

fn check(project: &Project, check: &Value) -> Res<()> {
  let subject = check["subject"].as_str().ok_or("subject")?;
  if subject == "regular" {
    let path = check["path"].as_str().ok_or("regular path")?;
    let file = EditedFile {
      path: path.to_owned(),
      absolute: project.repo.join(path),
      removed: false,
    };
    let expected = check["expected"].as_bool().ok_or("regular expected")?;
    return (is_regular_file(&file) == expected)
      .then_some(())
      .ok_or_else(|| format!("{check}"));
  }
  let file = file_for_call(project, &check["call"])?;
  let field = check["field"].as_str().unwrap_or("undefined");
  let observed = observed(project, file.as_ref(), field);
  let expected = (field != "undefined").then(|| project.value(&check["expected"]));
  (observed == expected)
    .then_some(())
    .ok_or_else(|| format!("{check}"))
}

#[test]
fn quality_edit_replays_every_shared_edit_case() {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/quality/runner.json");
  let text = std::fs::read_to_string(path).unwrap();
  let fixture: Value = serde_json::from_str(&text).unwrap();
  let cases = fixture["cases"].as_array().unwrap();
  let mut count = 0;
  for case in cases.iter().filter(|case| case["kind"] == "edit") {
    let project = Project::new().unwrap();
    project.setup(case).unwrap();
    for item in case["checks"].as_array().unwrap() {
      if item["subject"] == "linked" {
        let path = project.value(&item["path"]);
        let path = path.as_str().unwrap();
        let file = EditedFile {
          path: path.to_owned(),
          absolute: project.repo.join(path),
          removed: false,
        };
        assert_eq!(
          in_linked_worktree(
            &file,
            &RuleContext {
              host: toolu_protocol::host::Host::Claude,
              env: &Env::from_pairs([
                ("HOME", project.home.to_string_lossy().into_owned()),
                ("PATH", "/usr/bin:/bin".to_owned())
              ]),
              config_root: &project.home,
              project_root: &project.repo,
              cwd: Some(&project.repo),
              plugin_root: None,
              raw: &Map::new(),
              edit: None,
            }
          ),
          item["expected"].as_bool().unwrap(),
          "{}",
          case["name"]
        );
      } else {
        check(&project, item).unwrap();
      }
      count += 1;
    }
  }
  assert!(count >= 15, "shared edit cases did not load");
}
