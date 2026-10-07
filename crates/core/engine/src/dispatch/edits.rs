//! The edit split (`dispatchInput` and `dispatchRecords` in `dispatch.ts`): an
//! edit tool is walked once per affected path as a synthetic `Edit`, and each
//! path's settled output is folded into an outer walk, so a deny, block or exit 2
//! on any path wins for the whole patch.

use serde_json::Value;
use toolu_runtime::json::jq_text;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::edit_records::{EditRecord, EditRecords, normalize_edit_records};

use super::event::{EditFields, Payload};
use super::fold::{Folded, WalkState};
use super::output::{Unreadable, parse_document, read_field, sanitize_surrogates, substituted};
use super::session::Session;
use super::walk::walk;
use super::{ModuleResult, Phase};
use crate::trace::Step;

/// `toolu_dispatch_hook`'s fixed reply when `apply_patch` headers do not parse.
pub(crate) const MALFORMED_PATCH_DENY: &str = "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"Unable to parse apply_patch file headers; patch blocked so protected-file and quality gates cannot be bypassed.\"}}\n";

/// The `PostToolUse` counterpart of [`MALFORMED_PATCH_DENY`].
pub(crate) const MALFORMED_PATCH_BLOCK: &str = "{\"decision\":\"block\",\"reason\":\"Unable to parse apply_patch file headers; per-file post-edit quality checks could not run.\"}\n";

/// Why a payload nested past serde's recursion limit is refused.
const TOO_DEEP: &str = "toolu: the hook payload nests too deeply to be checked";

fn reply(stdout: String) -> ModuleResult {
  ModuleResult {
    stdout,
    ..ModuleResult::default()
  }
}

/// A fixed deny before the tool, or block after it, for `reason`.
fn refusal(phase: Phase, reason: &str) -> ModuleResult {
  let reason = Value::from(reason).to_string();
  reply(match phase {
    Phase::Pre => format!(
      "{{\"hookSpecificOutput\":{{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":{reason}}}}}\n"
    ),
    Phase::Post => format!("{{\"decision\":\"block\",\"reason\":{reason}}}\n"),
  })
}

/// One hook call's input, after its trailing newlines were stripped.
pub(crate) fn dispatch_input(
  text: &str,
  session: &Session<'_>,
  trace: &mut Vec<Step>,
) -> ModuleResult {
  let doc = match parse_document(text) {
    Ok(doc) => Some(doc),
    Err(Unreadable::TooDeep) => return refusal(session.phase, TOO_DEEP),
    Err(Unreadable::NotJson) => None,
  };
  let tool_name = read_field(doc.as_ref(), &["tool_name"]);
  let value = serde_json::from_str::<Value>(&sanitize_surrogates(text)).unwrap_or(Value::Null);
  let malformed = || match session.phase {
    Phase::Pre => reply(MALFORMED_PATCH_DENY.to_owned()),
    Phase::Post => reply(MALFORMED_PATCH_BLOCK.to_owned()),
  };
  match (normalize_edit_records(&value, &tool_name), doc) {
    (EditRecords::NotEdit, _) => {
      let payload = Payload {
        text: text.to_owned(),
        tool_name,
        edit: None,
      };
      walk(&payload, session, trace)
    }
    (EditRecords::Records(records), Some(doc @ Ordered::Object(_))) if !records.is_empty() => {
      fold_records(&doc, &records, session, trace)
    }
    (EditRecords::Malformed | EditRecords::Records(_), _) => malformed(),
  }
}

/// The synthetic single-path `Edit` payload `toolu_dispatch_hook` builds with `jq -c`.
fn synthetic_edit(doc: &Ordered, record: &EditRecord) -> String {
  let mut input = match doc.get("tool_input") {
    Some(input @ Ordered::Object(_)) => input.clone(),
    Some(_) | None => Ordered::Object(Vec::new()),
  };
  let text = |value: &str| Ordered::String(value.to_owned());
  input.set("file_path", text(&record.path));
  input.set("path", text(&record.path));
  input.set("toolu_edit_operation", text(record.operation.name()));
  input.set(
    "toolu_edit_from",
    text(record.from.as_deref().unwrap_or_default()),
  );
  input.set(
    "toolu_edit_moved_to",
    text(record.moved_to.as_deref().unwrap_or_default()),
  );
  let mut out = doc.clone();
  out.set("tool_name", text("Edit"));
  out.set("tool_input", input);
  jq_text(&out, false)
}

/// Walk every record, sequentially by contract: no path is walked after a deny.
fn fold_records(
  doc: &Ordered,
  records: &[EditRecord],
  session: &Session<'_>,
  trace: &mut Vec<Step>,
) -> ModuleResult {
  let collect_blocks = session.phase == Phase::Post && session.options.continue_post_blocks;
  let mut outer = WalkState::new(session.phase);
  let mut blocks: Vec<String> = Vec::new();
  for record in records {
    let payload = Payload {
      text: synthetic_edit(doc, record),
      tool_name: "Edit".to_owned(),
      edit: Some(EditFields::of(record)),
    };
    let result = walk(&payload, session, trace);
    outer.stderr.push_str(&result.stderr);
    if collect_blocks && result.exit_code != 0 {
      return ModuleResult {
        stderr: outer.stderr,
        ..result
      };
    }
    let parsed = parse_document(&result.stdout).ok();
    if collect_blocks && read_field(parsed.as_ref(), &["decision"]) == "block" {
      let reason = read_field(parsed.as_ref(), &["reason"]);
      blocks.push(if reason.is_empty() {
        "check blocked".to_owned()
      } else {
        reason
      });
      continue;
    }
    let folded = Folded {
      name: String::new(),
      result: ModuleResult {
        stdout: substituted(&result.stdout).to_owned(),
        stderr: String::new(),
        exit_code: result.exit_code,
      },
      truncated: false,
    };
    if let Some(done) = outer.consume(&folded) {
      return done;
    }
  }
  if blocks.is_empty() {
    return outer.settle();
  }
  joined_block(&blocks, outer.stderr)
}

/// Every collected block reason in one `{"decision":"block"}` line.
fn joined_block(blocks: &[String], stderr: String) -> ModuleResult {
  let block = Ordered::Object(vec![
    ("decision".to_owned(), Ordered::String("block".to_owned())),
    ("reason".to_owned(), Ordered::String(blocks.join("\n\n"))),
  ]);
  ModuleResult {
    stdout: format!("{}\n", jq_text(&block, false)),
    stderr,
    exit_code: 0,
  }
}

#[cfg(test)]
#[path = "tests/edits_test.rs"]
mod tests;
