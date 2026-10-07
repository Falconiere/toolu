//! `preflight` on real plan and spec docs, on each host (`ledger.test.ts`).

use crate::ledger::context::test_repo::Repo;
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;

use super::ledger_preflight;
use crate::ledger::io::LedgerOptions;

fn docs(repo: &Repo, plan: &str, spec: &str) {
  std::fs::write(
    repo.root.join("spec.md"),
    format!("# S\n\n**Date:** 2031-02-03   **Status:** {spec}\n"),
  )
  .unwrap();
  std::fs::write(
    repo.root.join("p.md"),
    format!("# P\n\n**Status:** {plan}   **Spec:** spec.md\n"),
  )
  .unwrap();
}

fn on(repo: &Repo, host: Host) -> LedgerOptions {
  LedgerOptions {
    roots: Roots::new(repo.env(), Some(host)),
    ..repo.opts()
  }
}

fn preflight(opts: &LedgerOptions, plan: Option<&str>) -> (u8, String) {
  let result = ledger_preflight(plan, opts);
  (result.exit, result.stderr)
}

#[test]
fn each_host_names_its_delivery_flow_remedy() {
  let repo = Repo::new().unwrap();
  for (host, action) in [
    (
      Host::Opencode,
      "load skill({ name: \"delivery-flow-delivery-flow\" })",
    ),
    (Host::Claude, "run /delivery-flow:delivery-flow"),
    (Host::Codex, "run /delivery-flow:delivery-flow"),
    (Host::Cursor, "load the delivery-flow skill"),
    (Host::Hermes, "load the delivery-flow skill"),
  ] {
    let opts = on(&repo, host);
    docs(&repo, "Draft", "Approved");
    let plan =
      format!("preflight: plan not approved (Status: Draft) — {action} (plan review phase)\n");
    assert_eq!(preflight(&opts, Some("p.md")), (1, plan));
    docs(&repo, "approved", "Draft");
    let spec = format!(
      "preflight: spec spec.md not approved (Status: Draft) — {action} (spec review phase)\n"
    );
    assert_eq!(preflight(&opts, Some("p.md")), (1, spec));
    docs(&repo, "Approved", "APPROVED");
    assert_eq!(preflight(&opts, Some("p.md")), (0, String::new()));
  }
}

#[test]
fn missing_headers_docs_and_specs_are_refused() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  assert_eq!(
    preflight(&opts, Some("absent.md")),
    (
      2,
      "preflight: plan doc not found or unreadable: absent.md\n".to_owned()
    )
  );
  std::fs::write(repo.root.join("p.md"), "# P\n").unwrap();
  let header = "preflight: plan has no **Status:** header (p.md) — run /delivery-flow:delivery-flow (plan review phase) to stamp it\n";
  assert_eq!(preflight(&opts, Some("p.md")), (1, header.to_owned()));
  std::fs::write(
    repo.root.join("p.md"),
    "**Status:** Approved **Spec:** gone.md\n",
  )
  .unwrap();
  assert_eq!(
    preflight(&opts, Some("p.md")),
    (
      1,
      "preflight: declared spec not found or unreadable: gone.md\n".to_owned()
    )
  );
  std::fs::write(
    repo.root.join("p.md"),
    "**Status:** Approved **Spec:** spec.md\n",
  )
  .unwrap();
  std::fs::write(repo.root.join("spec.md"), "# no status\n").unwrap();
  let shown = "preflight: spec spec.md not approved (Status: none) — run /delivery-flow:delivery-flow (spec review phase)\n";
  assert_eq!(preflight(&opts, Some("p.md")), (1, shown.to_owned()));
  std::fs::write(
    repo.root.join("p.md"),
    "**Status:** Approved **Spec:** None\n",
  )
  .unwrap();
  assert_eq!(preflight(&opts, Some("p.md")), (0, String::new()));
}

#[test]
fn without_a_doc_the_ledger_plan_is_checked_and_its_orphans_healed() {
  let repo = Repo::new().unwrap();
  let opts = repo.opts();
  let none = "preflight: no plan doc given and no ledger plan_doc to resolve\n";
  assert_eq!(preflight(&opts, None), (2, none.to_owned()));
  docs(&repo, "Approved", "Approved");
  let ledger = r#"{"plan_doc":"p.md","steps":[{"id":"s1","status":"running","started_at":"2031-02-03T03:00:00Z"}]}"#;
  repo.sh(&format!("mkdir -p .claude/tmp/plan-ledger && printf '%s' '{ledger}' > .claude/tmp/plan-ledger/feat_x.json")).unwrap();
  assert_eq!(preflight(&opts, Some("")), (0, String::new()));
  let healed =
    std::fs::read_to_string(repo.root.join(".claude/tmp/plan-ledger/feat_x.json")).unwrap();
  assert!(healed.contains("\"status\": \"pending\""), "{healed}");
}
