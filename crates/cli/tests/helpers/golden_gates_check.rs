//! Running one case and judging it the way the deleted TypeScript suites did: the
//! decision class and text, then the `comparable` projection against the golden capture.

use serde_json::{Value, json};

use crate::golden_gates_corpus::run_corpus;
use crate::golden_gates_files::{snapshot, touched};
use crate::golden_gates_model::{Capture, CorpusCase, Host, ModuleCase, Res};
use crate::golden_gates_prepare::{Suite, prepare_a, prepare_gate};
use crate::golden_gates_run::{Call, normalized, run_hook};
use crate::golden_gates_sandbox::Sandbox;

/// Run `c` in a fresh sandbox, the sandbox root written `$ROOT`; C also records the
/// files the hook wrote.
pub(crate) fn run_case(suite: Suite, c: &ModuleCase) -> Res<Capture> {
  let sb = Sandbox::new("main")?;
  let prepared = match suite {
    Suite::A => prepare_a(&sb, c)?,
    Suite::B | Suite::C => prepare_gate(&sb, c, suite)?,
  };
  let before = if suite == Suite::C {
    Some(snapshot(&sb.root)?)
  } else {
    None
  };
  let call = Call {
    cwd: &prepared.cwd,
    env: &prepared.env,
    stdin: &prepared.stdin,
  };
  let got = normalized(run_hook(&c.entry, &call)?, &sb.root);
  match before {
    Some(before) => {
      let files = touched(&before, &snapshot(&sb.root)?, &sb.root)?;
      Ok(Capture {
        files: Some(files),
        ..got
      })
    }
    None => Ok(got),
  }
}

fn parse(stdout: &str) -> Res<Option<Value>> {
  if stdout.trim().is_empty() {
    return Ok(None);
  }
  Ok(Some(serde_json::from_str(stdout)?))
}

/// What must match a capture. A: the decision JSON, the exit code, and stderr without
/// `toolu-config:` lines. B and C: the decision JSON (not its whitespace), exit code, stderr,
/// and for C the touched files.
pub(crate) fn comparable(suite: Suite, capture: &Capture) -> Res<Value> {
  let parsed = parse(&capture.stdout)?;
  if suite == Suite::A {
    let stderr: Vec<&str> = capture
      .stderr
      .split('\n')
      .filter(|line| !line.starts_with("toolu-config: "))
      .collect();
    return Ok(json!({
      "decision": parsed.unwrap_or(Value::Null),
      "stderr": stderr.join("\n"),
      "exitCode": capture.exit_code,
    }));
  }
  let mut out = json!({
    "stdout": parsed.unwrap_or_else(|| Value::String(String::new())),
    "stderr": capture.stderr,
    "exitCode": capture.exit_code,
  });
  if let (Some(files), Some(map)) = (&capture.files, out.as_object_mut()) {
    map.insert("files".to_owned(), serde_json::to_value(files)?);
  }
  Ok(out)
}

/// The decision class of a hook's stdout and its text (the reason, else the context).
fn decision_of(stdout: &str) -> Res<(String, String)> {
  let Some(out) = parse(stdout)? else {
    return Ok(("silent".to_owned(), String::new()));
  };
  let text_at = |value: Option<&Value>, key: &str| {
    value
      .and_then(|v| v.get(key))
      .and_then(Value::as_str)
      .map(str::to_owned)
  };
  let hso = out.get("hookSpecificOutput");
  let permission = text_at(hso, "permissionDecision");
  if let Some(kind) = permission.filter(|kind| kind == "deny" || kind == "ask") {
    return Ok((
      kind,
      text_at(hso, "permissionDecisionReason").unwrap_or_default(),
    ));
  }
  let context = text_at(hso, "additionalContext").or_else(|| text_at(Some(&out), "systemMessage"));
  Ok(("advisory".to_owned(), context.unwrap_or_default()))
}

/// A problem with the decision's class or text, if any.
fn intent(c: &ModuleCase, got: &Capture) -> Res<Option<String>> {
  let (outcome, text) = decision_of(&got.stdout)?;
  if outcome != c.expect {
    return Ok(Some(format!(
      "expected a {} decision, got {outcome}: {text}",
      c.expect
    )));
  }
  let lacking = c.has.iter().find(|part| !text.contains(part.as_str()));
  if let Some(part) = lacking {
    return Ok(Some(format!("decision lacks {part:?}: {text}")));
  }
  let present = c.lacks.iter().find(|part| text.contains(part.as_str()));
  Ok(present.map(|part| format!("decision has {part:?}: {text}")))
}

fn same(suite: Suite, deviation: bool, got: &Value, want: &Value) -> bool {
  match (deviation, suite) {
    (false, _) => got == want,
    (true, Suite::A) => got.get("decision") != want.get("decision"),
    (true, _) => got != want,
  }
}

/// `None` when `c` replays its golden `want`, else what differs.
pub(crate) fn check_case(suite: Suite, c: &ModuleCase, want: &Capture) -> Res<Option<String>> {
  let got = run_case(suite, c)?;
  if let Some(problem) = intent(c, &got)? {
    return Ok(Some(problem));
  }
  let (got_view, want_view) = (comparable(suite, &got)?, comparable(suite, want)?);
  if same(suite, c.deviation.is_some(), &got_view, &want_view) {
    return Ok(None);
  }
  let how = if c.deviation.is_some() {
    "must differ from the known-wrong golden"
  } else {
    "differs"
  };
  Ok(Some(format!(
    "{how}\n  golden: {want_view}\n  native: {got_view}"
  )))
}

/// The class `pretool-corpus.json` names: `exit2`, `silent`, `deny`, `ask` or `advisory`.
fn outcome_of(capture: &Capture) -> Res<&'static str> {
  if capture.exit_code == 2 {
    return Ok("exit2");
  }
  let Some(out) = parse(&capture.stdout)? else {
    return Ok("silent");
  };
  let text = out.to_string();
  Ok(if text.contains(r#""permissionDecision":"deny""#) {
    "deny"
  } else if text.contains(r#""permissionDecision":"ask""#) {
    "ask"
  } else {
    "advisory"
  })
}

/// `None` when corpus row `c` as `host` replays its golden `want`, else what differs. B
/// also holds the row's expected class.
pub(crate) fn check_corpus(
  suite: Suite,
  c: &CorpusCase,
  host: Host,
  want: &Capture,
) -> Res<Option<String>> {
  let got = run_corpus(c, host)?;
  let (got_view, want_view) = (comparable(suite, &got)?, comparable(suite, want)?);
  if got_view != want_view {
    return Ok(Some(format!(
      "differs\n  golden: {want_view}\n  native: {got_view}"
    )));
  }
  if suite == Suite::B {
    let expected = match host {
      Host::Codex => c.expect.codex.as_deref().unwrap_or(&c.expect.claude),
      Host::Claude => &c.expect.claude,
    };
    let outcome = outcome_of(&got)?;
    if outcome != expected {
      return Ok(Some(format!("expected {expected}, got {outcome}")));
    }
  }
  Ok(None)
}
