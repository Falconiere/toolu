//! Page tests for the worker drawer: regions, labels, notices and polling gates.

use std::path::Path;

fn read_asset(file: &str) -> String {
  let path = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("assets/dashboard")
    .join(file);
  std::fs::read_to_string(&path).expect(file)
}

#[test]
fn dashboard_drawer_has_regions_step_marks_notices_and_attach_block() {
  let page = read_asset("index.html");
  for id in [
    "workspace",
    "drawer",
    "drawer-header",
    "drawer-status",
    "drawer-attention",
    "drawer-plan",
    "drawer-steps",
    "drawer-screen",
    "drawer-screen-text",
    "drawer-blocked",
    "drawer-copy",
    "drawer-phases",
    "drawer-actions",
  ] {
    assert!(page.contains(&format!("id=\"{id}\"")), "{id}");
  }
  let version = env!("CARGO_PKG_VERSION");
  assert!(page.contains(&format!("/assets/{version}/drawer.css")));
  for text in [
    "▲ BLOCKED · WAITING ON A KEY PRESS",
    "Key presses from the web are not supported. Answer it here, or attach from a terminal:",
    "[ COPY ]",
  ] {
    // oxfmt wraps long text lines in the HTML.
    let flat = page.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains(text), "{text}");
  }
}

#[test]
fn dashboard_drawer_logic_has_step_marks_notices_and_attach_form() {
  let logic = read_asset("js/logic/drawer.js");
  for (status, label) in [
    ("green", "✓ GREEN"),
    ("running", "● IN PROGRESS"),
    ("pending", "○ TODO"),
    ("red", "✕ FAILING"),
    ("stale", "↺ STALE"),
  ] {
    assert!(
      logic.contains(&format!(
        "{status}: {{ kind: \"{status}\", label: \"{label}\" }}"
      )),
      "{status}"
    );
  }
  for notice in [
    "○ CONTROL LOCKED",
    "Screen reads need the token. Open the token URL the engine printed.",
    "/ AGENT NOT RUNNING",
    "The screen appears once a host claims it.",
    "? LIVE VIEW UNAVAILABLE",
    "herdr not reachable. Plans and phases still update.",
    "● PLAN UNREADABLE",
    "The ledger failed to parse. Phase history shown instead.",
    "◇ OPEN DASHBOARD TO WATCH OR CONTROL",
    "Screens, live status and actions stay on the peer. This view shows the plan only.",
    "No plan yet. Phase history below until the worker writes its ledger.",
  ] {
    assert!(logic.contains(notice), "{notice}");
  }
  assert!(
    logic.contains("body?.agent_status === \"blocked\" && typeof body.attach === \"string\"")
  );
  let tests = read_asset("tests/drawer.test.js");
  assert!(tests.contains("herdr agent attach toolu-651"));
  assert!(tests.contains("herdr --session work agent attach"));
  let view = read_asset("js/view/drawer.js");
  assert!(view.contains("`$ ${attach}`") && view.contains("COPIED ✓"));
  assert!(view.contains("DASHBOARD ↗ ]`"));
  assert!(view.contains("`Worker ${model.key}`"));
}

#[test]
fn dashboard_drawer_polls_only_while_open_and_visible() {
  let control = read_asset("js/worker-drawer.js");
  assert!(control.contains("const POLL_MS = 2000;"));
  assert!(
    control.contains(
      "if (!key || inflight === seq || document.visibilityState !== \"visible\") return;"
    )
  );
  assert!(control.contains("drawer.timer = setInterval(tick, POLL_MS);"));
  assert!(control.contains("clearInterval(drawer.timer);"));
  assert_eq!(
    control.matches("setInterval(").count(),
    1,
    "one drawer timer"
  );
  assert!(control.contains("wantsScreen({ peer, issue:"));
  assert!(control.contains("event.key !== \"Escape\""));
  let logic = read_asset("js/logic/drawer.js");
  assert!(
    logic.contains("!context.peer && !stoppedNotice(context.issue) && Boolean(context.token)")
  );
  let app = read_asset("js/app.js");
  assert!(app.contains("takeToken(location.hash)") && app.contains("history.replaceState"));
  let api = read_asset("js/api.js");
  assert!(api.contains("headers: { Authorization: `Bearer ${token}` }"));
  assert!(!api.contains("screen?token") && !api.contains("token=${"));
}
