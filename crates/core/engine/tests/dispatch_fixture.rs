//! The shared dispatch fixture (AC-1, AC-2, AC-4, AC-10): the engine reproduces
//! every case of `fixtures/dispatch/cases.json` byte for byte, as the TypeScript
//! dispatcher does (`packages/toolu-core/src/dispatch/__tests__/dispatch-fixture.test.ts`).
//! Stdin objects are printed in the fixture's key order, as `JSON.stringify` does.

#[path = "helpers/hook.rs"]
mod hook;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::path::PathBuf;

use hook::Hook;
use sandbox::{Res, write};
use serde_json::Value;
use toolu_engine::Phase;
use toolu_engine::gate::Gate;
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::registry::rule::RuleContext;

const FIXTURE: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../../fixtures/dispatch/cases.json"
);

/// A fixture built-in: a fixed decision, or a thrown error.
struct Builtin {
  name: String,
  answer: Result<Decision, String>,
}

impl Gate for Builtin {
  fn name(&self) -> &str {
    &self.name
  }

  fn run(&self, _event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Result<Decision, String> {
    self.answer.clone()
  }
}

/// The sandbox paths the case tokens stand for.
struct Paths(Vec<(&'static str, String)>);

impl Paths {
  fn of(hook: &Hook) -> Paths {
    Paths(vec![
      ("PROJECT", hook.sb.text("project")),
      ("HOME", hook.sb.text("home")),
      ("CODEX", hook.sb.text("codex")),
      ("CONFIG", hook.config_root().to_string_lossy().into_owned()),
      ("LIB", hook.sb.text("plugin/hooks/lib")),
    ])
  }

  /// `$TOKEN` expanded, unless a longer name such as `$PROJECT_ROOT` goes on.
  fn expand(&self, text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('$') {
      let (before, after) = rest.split_at(at);
      out.push_str(before);
      let tail = after.get(1..).unwrap_or_default();
      let hit = self.0.iter().find(|(token, _)| {
        tail.starts_with(token)
          && !tail[token.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
      });
      match hit {
        Some((token, path)) => {
          out.push_str(path);
          rest = &tail[token.len()..];
        }
        None => {
          out.push('$');
          rest = tail;
        }
      }
    }
    out.push_str(rest);
    out
  }

  fn expand_ordered(&self, value: &Ordered) -> Ordered {
    match value {
      Ordered::String(text) => Ordered::String(self.expand(text)),
      Ordered::Array(items) => {
        Ordered::Array(items.iter().map(|item| self.expand_ordered(item)).collect())
      }
      Ordered::Object(entries) => Ordered::Object(
        entries
          .iter()
          .map(|(key, item)| (key.clone(), self.expand_ordered(item)))
          .collect(),
      ),
      other @ (Ordered::Null | Ordered::Bool(_) | Ordered::Number(_)) => other.clone(),
    }
  }
}

fn text<'v>(case: &'v Value, key: &str) -> Res<&'v str> {
  case
    .get(key)
    .and_then(Value::as_str)
    .ok_or_else(|| format!("no {key}"))
}

fn builtins(case: &Value) -> Res<Vec<Builtin>> {
  let list = case
    .get("builtins")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  list
    .iter()
    .map(|item| {
      let name = text(item, "name")?.to_owned();
      let answer = match (
        item.get("decision"),
        item.get("throws").and_then(Value::as_str),
      ) {
        (Some(decision), _) => {
          Ok(serde_json::from_value(decision.clone()).map_err(|err| err.to_string())?)
        }
        (None, Some(message)) => Err(message.to_owned()),
        (None, None) => return Err(format!("built-in {name} has no answer")),
      };
      Ok(Builtin { name, answer })
    })
    .collect()
}

/// Writes the case's registry, config and install record.
fn setup(hook: &mut Hook, case: &Value, phase: Phase, paths: &Paths) -> Res<()> {
  for module in case
    .get("registry")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default()
  {
    let file = text(&module, "file")?;
    match (
      module.get("sh").and_then(Value::as_str),
      module.get("js").and_then(Value::as_str),
    ) {
      (Some(body), _) => hook.sh(phase, file, &paths.expand(body))?,
      (None, Some(source)) => write(&hook.dir(phase).join(file), &paths.expand(source))?,
      (None, None) => return Err(format!("module {file} has no body")),
    }
  }
  let host_dir = if hook.host == Host::Codex {
    ".codex"
  } else {
    ".claude"
  };
  if let Some(config) = case.get("config") {
    write(
      &hook
        .sb
        .path(&format!("project/{host_dir}/toolu.config.json")),
      &config.to_string(),
    )?;
  }
  let installed = case.get("installed").and_then(Value::as_array);
  let record = hook.sb.path("home/.claude/plugins/installed_plugins.json");
  match (installed, hook.host) {
    (Some(specs), Host::Codex) => {
      let snapshot = serde_json::json!({"version": 1, "status": "ready", "plugins": specs});
      write(
        &hook.config_root().join("toolu/codex-plugins.json"),
        &snapshot.to_string(),
      )?;
    }
    (Some(specs), _) => {
      let plugins: serde_json::Map<String, Value> = specs
        .iter()
        .filter_map(Value::as_str)
        .map(|spec| (spec.to_owned(), serde_json::json!([{"scope": "user"}])))
        .collect();
      write(
        &record,
        &serde_json::json!({"version": 2, "plugins": plugins}).to_string(),
      )?;
    }
    (None, _) => {}
  }
  if let Some(raw) = case.get("installedRaw").and_then(Value::as_str) {
    write(&record, raw)?;
  }
  hook.continue_blocks = case.get("continuePostBlocks") == Some(&Value::Bool(true));
  Ok(())
}

/// Runs one case; the failure message when it differs from its capture.
fn check(case: &Value, stdin: &Ordered) -> Res<()> {
  let host = if text(case, "host")? == "codex" {
    Host::Codex
  } else {
    Host::Claude
  };
  let phase = if text(case, "phase")? == "pre" {
    Phase::Pre
  } else {
    Phase::Post
  };
  let mut hook = Hook::new(host)?;
  let paths = Paths::of(&hook);
  setup(&mut hook, case, phase, &paths)?;
  let stdin = match stdin {
    Ordered::String(raw) => paths.expand(raw),
    other => paths.expand_ordered(other).to_text(false),
  };
  let gates = builtins(case)?;
  let gates: Vec<&dyn Gate> = gates.iter().map(|gate| gate as &dyn Gate).collect();
  let out = hook.run(phase, &stdin, &gates, &[]).result;
  let expect = case.get("expect").ok_or("no expect")?;
  let want = (
    paths.expand(text(expect, "stdout")?),
    paths.expand(text(expect, "stderr")?),
    expect
      .get("exitCode")
      .and_then(Value::as_i64)
      .ok_or("no exitCode")?,
  );
  let got = (out.stdout, out.stderr, i64::from(out.exit_code));
  if got == want {
    Ok(())
  } else {
    Err(format!("got {got:?}\nwant {want:?}"))
  }
}

#[test]
fn the_engine_reproduces_every_dispatch_case() {
  let source = std::fs::read_to_string(PathBuf::from(FIXTURE)).unwrap();
  let doc: Value = serde_json::from_str(&source).unwrap();
  let ordered = Ordered::parse(&source).unwrap();
  let cases = doc["cases"].as_array().unwrap();
  let Some(Ordered::Array(stdins)) = ordered.get("cases").map(|cases| match cases {
    Ordered::Array(items) => Ordered::Array(
      items
        .iter()
        .filter_map(|case| case.get("stdin").cloned())
        .collect(),
    ),
    other => other.clone(),
  }) else {
    panic!("cases is not an array");
  };
  assert_eq!(cases.len(), stdins.len());
  assert!(cases.len() >= 50, "{} cases", cases.len());
  let failures: Vec<String> = cases
    .iter()
    .zip(&stdins)
    .filter_map(|(case, stdin)| {
      check(case, stdin)
        .err()
        .map(|why| format!("{}: {why}", case["name"]))
    })
    .collect();
  assert!(
    failures.is_empty(),
    "{} failed:\n{}",
    failures.len(),
    failures.join("\n\n")
  );
}
