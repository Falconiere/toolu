//! The delivery-flow skill Markdown carries the delivery contract: phases, ledger commands,
//! review verdict and PR delivery wording. Ported from the retired Bun contract suites.
//! Phrase lists are `|` separated; `SKILL` names the skill file, any other name a reference.

use std::path::{Path, PathBuf};

use regex::Regex;

type Res = Result<(), String>;

const SKILLS: &str = "plugins/delivery-flow/skills";
const REFS: &str = "plugins/delivery-flow/skills/delivery-flow/references";
const VERIFY: &str = "toolu ledger run <plan_doc> --verify";
const PHASES: &str = "spec|spec-review|plan|plan-review|execution|test";
const SECTIONS: &str = "Failure modes and edge cases|Acceptance evidence|Documentation impact";

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> Result<String, String> {
  let path = repo().join(rel);
  std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))
}

fn doc(name: &str) -> Result<String, String> {
  match name {
    "SKILL" => read(&format!("{SKILLS}/delivery-flow/SKILL.md")),
    _ => read(&format!("{REFS}/{name}.md")),
  }
}

fn exists(rel: &str) -> bool {
  repo().join(rel).exists()
}

fn require(ok: bool, what: impl Into<String>) -> Res {
  ok.then_some(())
    .ok_or_else(|| format!("expected: {}", what.into()))
}

fn scan(name: &str, list: &str, want: bool) -> Res {
  let body = doc(name)?;
  for part in list.split('|') {
    let what = format!("{name} {want}: {part}");
    require(body.contains(part) == want, what)?;
  }
  Ok(())
}

fn has(name: &str, list: &str) -> Res {
  scan(name, list, true)
}

fn lacks(name: &str, list: &str) -> Res {
  scan(name, list, false)
}

fn matched(text: &str, pattern: &str, want: bool) -> Res {
  let regex = Regex::new(pattern).map_err(|err| format!("{pattern}: {err}"))?;
  require(regex.is_match(text) == want, format!("/{pattern}/ {want}"))
}

fn re(name: &str, patterns: &[&str]) -> Res {
  let body = doc(name)?;
  patterns.iter().try_for_each(|p| matched(&body, p, true))
}

fn not_re(name: &str, patterns: &[&str]) -> Res {
  let body = doc(name)?;
  patterns.iter().try_for_each(|p| matched(&body, p, false))
}

fn skill_files(dir: &Path, found: &mut Vec<String>) -> Res {
  let entries = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    if path.is_dir() {
      skill_files(&path, found)?;
    } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
      found.push(path.display().to_string());
    }
  }
  Ok(())
}

/// 1-based number of the first line matching `pattern`, or 0 when none does.
fn line_of(body: &str, pattern: &str) -> Result<usize, String> {
  let regex = Regex::new(pattern).map_err(|err| format!("{pattern}: {err}"))?;
  let index = body.lines().position(|line| regex.is_match(line));
  Ok(index.map_or(0, |index| index + 1))
}

fn names_brainstorm(manifest: &str) -> Result<bool, String> {
  let json: serde_json::Value = serde_json::from_str(manifest).map_err(|err| err.to_string())?;
  let deps = json.get("dependencies").and_then(|value| value.as_array());
  let named = |dep: &serde_json::Value| dep.get("name") == Some(&"brainstorm".into());
  Ok(deps.is_some_and(|list| list.iter().any(named)))
}

// ---- delivery-flow-contract ----

#[test]
fn one_public_skill_owns_every_required_phase_and_private_guidance() -> Res {
  let mut found = Vec::new();
  skill_files(&repo().join(SKILLS), &mut found)?;
  let only = found.len() == 1 && found.iter().all(|f| f.ends_with("delivery-flow/SKILL.md"));
  require(only, format!("one SKILL.md, found {found:?}"))?;
  for phase in PHASES.split('|') {
    let what = format!("{phase} reference");
    require(exists(&format!("{REFS}/{phase}.md")), what)?;
    let public = exists(&format!("plugins/toolu/skills/{phase}/SKILL.md"));
    require(!public, format!("no toolu skill {phase}"))?;
    has("SKILL", &format!("references/{phase}.md"))?;
  }
  let list = "brainstorm → spec → spec review → plan → plan review → execution";
  has("SKILL", list)
}

#[test]
fn brainstorm_phase_runs_the_brainstorm_plugin_instead_of_a_private_copy() -> Res {
  for private in ["brainstorm", "design-questions"] {
    let what = format!("no {private}.md");
    require(!exists(&format!("{REFS}/{private}.md")), what)?;
  }
  has("SKILL", "brainstorm:brainstorm|installed `brainstorm`")?;
  for host in [".claude-plugin", ".codex-plugin"] {
    let manifest = read(&format!("plugins/delivery-flow/{host}/plugin.json"))?;
    let what = format!("{host} depends on brainstorm");
    require(names_brainstorm(&manifest)?, what)?;
  }
  Ok(())
}

#[test]
fn delivery_stops_at_failed_reviews_and_resumes_from_the_failed_phase() -> Res {
  let list = "Status: Needs changes|resume from that phase|toolu ledger preflight|real-data";
  has("SKILL", list)?;
  let list = "TOOLU_LIB|hooks/dist/plan-ledger.js|hooks/dist/verdict.js";
  lacks("SKILL", list)
}

#[test]
fn invocation_authorizes_checked_delivery_and_prerequisite_failures_block_it() -> Res {
  let list = "Invoking this skill authorizes|gh api user|non-default branch";
  has("SKILL", list)?;
  let list = "toolu-review:review|toolu ledger verdict status|overall: ready";
  has("SKILL", list)?;
  has("SKILL", "PR|pr-babysit:babysit")
}

// ---- execution-contract ----

#[test]
fn execution_verifies_every_ledger_step_against_the_final_branch_diff() -> Res {
  has("execution", VERIFY)?;
  re("execution", &["(?i)AC coverage"])
}

#[test]
fn execution_requires_per_step_real_data_evidence_and_documentation_sync() -> Res {
  let list = &["(?i)per-step.*real-data", "(?i)Docs in sync"];
  re("execution", list)
}

#[test]
fn execution_owns_the_v2_review_state_and_ready_unified_verdict() -> Res {
  let list = &["(?i)toolu-review.*version: 2|version: 2.*toolu-review"];
  re("execution", list)?;
  has("execution", "toolu ledger verdict status")?;
  re("execution", &["(?i)overall: ready"])
}

#[test]
fn execution_attests_the_committed_branch_before_delivery() -> Res {
  let body = doc("execution")?;
  require(body.matches(VERIFY).count() == 1, "one verify command")?;
  let order = [
    line_of(&body, "(?i)commit the scoped changes")?,
    line_of(&body, "toolu ledger run <plan_doc> --verify")?,
    line_of(&body, "toolu-review:review")?,
    line_of(&body, "toolu ledger verdict status")?,
    line_of(&body, "(?i)push the non-default feature branch")?,
  ];
  let what = "every delivery step is present";
  require(order.iter().all(|line| *line > 0), what)?;
  let what = "steps in strict order";
  require(
    order.windows(2).all(|pair| pair.first() < pair.get(1)),
    what,
  )
}

#[test]
fn execution_only_delivers_with_authorization_and_all_delivery_prerequisites() -> Res {
  re(
    "execution",
    &[
      "(?i)delivery authorization",
      "(?i)GitHub API authentication",
    ],
  )?;
  let list = &["(?i)non-default branch", "(?i)pr-babysit.*installed"];
  re("execution", list)
}

#[test]
fn execution_follows_the_required_brainstorm_phase() -> Res {
  re("execution", &["(?i)brainstorm.*already run"])
}

#[test]
fn execution_commits_pushes_creates_or_finds_a_default_branch_pr_then_invokes_babysit() -> Res {
  re(
    "execution",
    &[
      "(?i)commit.*scoped changes",
      "(?i)push",
      "(?i)repository default branch",
    ],
  )?;
  let list = &["(?i)locate.*or create.*pull request|create.*or locate.*pull request"];
  re("execution", list)?;
  let list = &["(?i)verify.*PR.*number.*head/base|verify.*head/base.*branches"];
  re("execution", list)?;
  has("execution", "pr-babysit:babysit")
}

#[test]
fn execution_has_no_execution_review_or_terminal_test_handoff() -> Res {
  not_re(
    "execution",
    &[
      "(?i)execution-review",
      "(?i)then to `test` for the final pass",
    ],
  )
}

#[test]
fn execution_uses_private_routing_and_host_neutral_delivery_invocations() -> Res {
  let list = "[model-routing.md](model-routing.md)|[host-mapping.md](host-mapping.md)";
  has("execution", list)?;
  let list = "plugins/toolu/skills/orchestrator/references/model-routing.md";
  lacks("execution", list)?;
  lacks("execution", "$toolu-review:review|$pr-babysit:babysit")
}

// ---- plan-ledger-contract ----

#[test]
fn plan_emits_a_machine_readable_steps_block() -> Res {
  has("plan", "Steps (machine-readable)")
}

#[test]
fn plan_review_asserts_every_step_has_a_runnable_check_and_rejects_empty_steps() -> Res {
  has("plan-review", "non-empty runnable|empty steps")
}

#[test]
fn execution_reads_status_and_records_each_step_via_run_plan_doc_step_id() -> Res {
  let list = "toolu ledger status|toolu ledger run <plan_doc>|--step <id>";
  has("execution", list)
}

#[test]
fn spec_documents_the_ac_id_convention() -> Res {
  has("spec", "AC-<n>")
}

#[test]
fn spec_authors_delivery_critical_evidence_and_impact_sections() -> Res {
  for section in SECTIONS.split('|') {
    re("spec", &[&format!("(?m)^## {section}$")])?;
  }
  Ok(())
}

#[test]
fn spec_review_validates_authored_evidence_and_non_blocking_questions() -> Res {
  has("spec-review", SECTIONS)?;
  has("spec-review", "observable real-data|non-blocking")
}

#[test]
fn shared_plan_reference_documents_optional_ac_refs_depends_on_input_fields() -> Res {
  has("ledger", "ac_refs|depends_on|input")
}

#[test]
fn shared_plan_reference_uses_the_supported_final_verification_command() -> Res {
  has("ledger", VERIFY)
}

#[test]
fn plan_is_evidence_first_and_keeps_detailed_steps_only_in_the_ledger() -> Res {
  let list = "Evidence first|Mechanical work|sole detailed step list|ledger.md";
  has("plan", list)?;
  require(exists(&format!("{REFS}/ledger.md")), "ledger.md")
}

#[test]
fn plan_delegates_the_full_ledger_schema_to_its_shared_reference() -> Res {
  lacks("plan", "each item has non-empty")?;
  has("plan", "canonical ledger shape")
}

#[test]
fn plan_review_asserts_ac_refs_resolve_via_pl_check_ac_refs() -> Res {
  has("plan-review", "ac_refs|pl_check_ac_refs")
}

#[test]
fn plan_review_checks_delivery_ready_evidence_and_coverage() -> Res {
  has(
    "plan-review",
    "AC-to-step coverage|real input|Path declarations|PR-delivery readiness",
  )
}

#[test]
fn test_is_an_execution_time_method_with_a_behavior_to_evidence_map() -> Res {
  let list = "reusable execution-time method|Behavior-to-evidence map";
  has("test", list)?;
  has("test", "mock-substitute|happy-path-only")?;
  lacks("test", "mocks/")
}

#[test]
fn workflow_docs_show_execution_time_real_data_tests() -> Res {
  let pattern = "(?i)execution with real-data tests";
  matched(&read("README.md")?, pattern, true)?;
  matched(&read("docs/toolu/README.md")?, pattern, true)
}

#[test]
fn execution_reads_ac_coverage_from_status() -> Res {
  has("execution", "AC-coverage")
}
