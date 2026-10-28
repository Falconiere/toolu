//! Page tests for the control UI: token handling, request headers, URLs,
//! the kind-to-action mapping, disable rules, labels and peer mode.

use std::path::Path;

/// One dashboard asset file as text; the drawer tests share it.
pub(super) fn read_asset(file: &str) -> String {
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("assets/dashboard")
    .join(file);
  std::fs::read_to_string(&path).expect(file)
}

/// `text` with every whitespace run collapsed, since oxfmt wraps long HTML lines.
pub(super) fn flat(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every served text asset with its name, plus the page.
pub(super) fn served_text() -> Vec<(String, String)> {
  let mut files = vec![(
    "index.html".to_owned(),
    String::from_utf8_lossy(super::PAGE).into_owned(),
  )];
  for (name, bytes, content_type) in super::ASSETS {
    if content_type != "font/woff2" {
      files.push((name.to_owned(), String::from_utf8_lossy(bytes).into_owned()));
    }
  }
  files
}

#[test]
fn dashboard_control_reads_the_fragment_then_removes_it() {
  let control = read_asset("js/control.js");
  assert!(control.contains("const fragment = takeToken(location.hash);"));
  assert!(control.contains(
    "history.replaceState(null, \"\", `${location.pathname}${location.search}${fragment.hash}`);"
  ));
  assert!(control.contains("window.addEventListener(\"hashchange\", storeToken);"));
  let fragment = read_asset("js/logic/fragment.js");
  assert!(fragment.contains("list.delete(\"token\");"));
}

#[test]
fn dashboard_control_keeps_the_token_only_in_session_storage() {
  for (name, text) in served_text() {
    assert!(!text.contains("localStorage"), "{name} uses localStorage");
    assert!(!text.contains("document.cookie"), "{name} uses cookies");
    if name != "js/control.js" {
      assert!(
        !text.contains("sessionStorage"),
        "{name} touches sessionStorage"
      );
    }
  }
  let control = read_asset("js/control.js");
  assert!(control.contains("return action(sessionStorage);"));
  assert!(control.contains("store.setItem(TOKEN_KEY, fragment.token)"));
  assert!(control.contains("store.removeItem(TOKEN_KEY)"));
}

#[test]
fn dashboard_control_actions_send_bearer_and_json() {
  let logic = read_asset("js/logic/control.js");
  assert!(logic.contains("path: `/api/v1/actions/${encodeURIComponent(op)}`,"));
  assert!(logic.contains("method: \"POST\","));
  assert!(logic.contains(
    "headers: { Authorization: `Bearer ${token}`, \"Content-Type\": \"application/json\" },"
  ));
  let api = read_asset("js/api.js");
  assert!(api.contains("const { path, init } = actionRequest(op, body, token);"));
  assert!(api.contains("const response = await fetch(path, init);"));
  // A press whose button a render moved still sends exactly one click.
  let control = read_asset("js/control.js");
  assert!(control.contains("if (!id || button?.dataset.control !== id) return;"));
  assert!(control.contains("if (!press.clicked) button.click();"));
}

#[test]
fn dashboard_control_builds_no_url_with_the_token() {
  for (name, text) in served_text() {
    for line in text.lines() {
      let builds_url = [
        "/api/v1",
        "fetch(",
        "EventSource(",
        "replaceState(",
        "href =",
      ]
      .iter()
      .any(|marker| line.contains(marker));
      assert!(
        !(builds_url && line.to_lowercase().contains("token")),
        "{name}: {line}"
      );
    }
  }
}

#[test]
fn dashboard_control_maps_engine_kinds_and_disable_rules() {
  let logic = read_asset("js/logic/control.js");
  assert!(logic.contains(
    "const ANSWER_KINDS = new Set([\"needs-human\", \"stuck\", \"help\", \"contract-changed\"]);"
  ));
  assert!(logic.contains("return kind === \"coupling-hold\" ? \"release\" : \"ack\";"));
  assert!(logic.contains("if (!key) return \"ack\";"));
  assert!(logic.contains("Boolean(locked || pending)"));
  assert!(logic.contains("disabled: off || !liveAgent(issue)"));
  assert!(logic.contains("Boolean(issue?.stage) && !DONE_STAGES.includes(issue.stage)"));
  assert!(logic.contains("if (issue?.hold) actions.push({ action: \"release\""));
  assert!(logic.contains("if (attentionOpen(attention)) actions.push({ action: \"ack\""));
  assert!(logic.contains("controlDisabled({ locked, pending }) || !String(text ?? \"\").trim()"));
  for text in [
    "○ CONTROL LOCKED",
    "Watching only. Open the token URL the engine printed to pause, resume, answer or release. Treat that URL like a password.",
    "● 401 · TOKEN ROTATED",
    "Control is locked. Nothing was paused. Open the new token URL to unlock.",
    "▲ The agent is working. The host may queue this or apply it mid-turn.",
    "✓ DELIVERED",
    "Attention marked answered.",
    "▲ UNCERTAIN",
    "Delivery uncertain; check the screen.",
    "The engine refused the answer. Attach from a terminal.",
    "The effect lands through the journal stream.",
  ] {
    assert!(logic.contains(text), "{text}");
  }
  let tests = read_asset("tests/control.test.js");
  assert!(tests.contains("\"● REFUSED · agent_blocked\""));
}

#[test]
fn dashboard_control_labels_every_control_and_the_dialog() {
  let page = read_asset("index.html");
  let flat = flat(&page);
  for text in [
    "PAUSE ALL EPICS",
    "RESUME ALL EPICS",
    "○ CONTROL LOCKED",
    "[ UNLOCK ]",
    "aria-label=\"Unlock control\"",
    "<dialog id=\"answer-dialog\" class=\"answer-dialog\" aria-labelledby=\"answer-title\">",
    "THE WORKER ASKED",
    "<label class=\"eyebrow\" for=\"answer-text\">YOUR ANSWER</label>",
    "maxlength=\"4000\"",
    "0 / 4000",
    "CANCEL",
    "SEND ANSWER →",
    "id=\"control-banner\" class=\"control-banner\" role=\"alert\"",
    "id=\"toast\" class=\"toast\" role=\"status\"",
  ] {
    assert!(flat.contains(text), "{text}");
  }
  assert!(!page.contains("Reserved for"));
  let view = read_asset("js/view/controls.js");
  assert!(view.contains("button.setAttribute(\"aria-label\", name);"));
  let dialog = read_asset("js/answer-dialog.js");
  assert!(dialog.contains("$(\"answer-dialog\").showModal();"));
  assert!(dialog.contains("$(\"answer-dialog\").addEventListener(\"close\", restoreFocus);"));
}

#[test]
fn dashboard_control_never_acts_from_a_peer() {
  let control = read_asset("js/control.js");
  assert!(control.contains("if (current().peer || !token || control.pending) return null;"));
  assert!(control.contains("if (peer || !docs) return;"));
  let view = read_asset("js/view/controls.js");
  assert!(view.contains("if (peer || !model.docs) {"));
  assert!(view.contains("$(id).hidden = peer;"));
  assert!(view.contains("toggle.hidden = Boolean(peer);"));
  let fetches: usize = served_text()
    .iter()
    .map(|(_, text)| text.matches("fetch(").count())
    .sum();
  assert_eq!(read_asset("js/api.js").matches("fetch(").count(), fetches);
}
