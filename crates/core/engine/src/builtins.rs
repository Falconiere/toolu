//! The built-in gate tables of `toolu hook pre-tools` and `toolu hook post-tools`
//! (`NATIVE_MODULES` in `plugins/toolu/hooks/src/pre-tools/builtins.ts` and
//! `BUILTIN_MODULES` in `post-tools/builtins.ts`), each in its TypeScript order.
//! #419 supplies code-edit-rules, mcp-blocker and protected-files; #420 and
//! #422 supply the remaining pre-tool gates.

use crate::gate::Gate;
use crate::gates::code_edit_rules::CODE_EDIT;
use crate::gates::gate_status::GATE_STATUS;
use crate::gates::mcp_blocker::MCP_BLOCKER;
use crate::gates::protected_files::PROTECTED;
use crate::gates::push_waiver::PUSH_WAIVER;

/// The `PreToolUse` built-ins, in table order.
pub const PRE_TOOL: &[&dyn Gate] = &[&CODE_EDIT, &MCP_BLOCKER, &PROTECTED];

/// The `PostToolUse` built-ins, in table order: gate status before push waiver.
pub const POST_TOOL: &[&dyn Gate] = &[&GATE_STATUS, &PUSH_WAIVER];
