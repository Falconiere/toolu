//! The built-in gate tables of `toolu hook pre-tools` and `toolu hook post-tools`
//! (`NATIVE_MODULES` in `plugins/toolu/hooks/src/pre-tools/builtins.ts` and
//! `BUILTIN_MODULES` in `post-tools/builtins.ts`), each in its TypeScript order.
//! #419 supplies code-edit-rules and protected-files; #420 and
//! #422 supply the remaining pre-tool gates. The mcp-blocker gate added by
//! #419 was retired by the maintainer and is no longer in the table.

use crate::gate::Gate;
use crate::gates::bash_commands::BASH_COMMANDS;
use crate::gates::code_edit_rules::CODE_EDIT;
use crate::gates::commit_gate::COMMIT_GATE;
use crate::gates::docs_sync::DOCS_SYNC;
use crate::gates::gate_status::GATE_STATUS;
use crate::gates::job_results::JOB_RESULTS;
use crate::gates::plan_ledger::PLAN_LEDGER;
use crate::gates::pr_size::PR_SIZE;
use crate::gates::protected_files::PROTECTED;
use crate::gates::push_review::PUSH_REVIEW;
use crate::gates::push_waiver::PUSH_WAIVER;
use crate::gates::quality_gate::QUALITY_GATE;

/// The `PreToolUse` built-ins, in table order.
pub const PRE_TOOL: &[&dyn Gate] = &[
  &BASH_COMMANDS,
  &CODE_EDIT,
  &COMMIT_GATE,
  &DOCS_SYNC,
  &PLAN_LEDGER,
  &PROTECTED,
  &PUSH_REVIEW,
  // After push-review, so its base-missing denial keeps the first word.
  &PR_SIZE,
  &QUALITY_GATE,
];

/// The `PostToolUse` built-ins, in table order: gate status before push waiver.
pub const POST_TOOL: &[&dyn Gate] = &[&GATE_STATUS, &PUSH_WAIVER, &JOB_RESULTS];

/// Claude reports failed shell calls separately. Only record quality failures.
pub const POST_TOOL_FAILURE: &[&dyn Gate] = &[&GATE_STATUS];
