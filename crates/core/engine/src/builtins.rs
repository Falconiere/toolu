//! The built-in gate tables of `toolu hook pre-tools` and `toolu hook post-tools`
//! (`NATIVE_MODULES` in `plugins/toolu/hooks/src/pre-tools/builtins.ts` and
//! `BUILTIN_MODULES` in `post-tools/builtins.ts`), each in its TypeScript order.
//! The pre-tool table stays empty until #419–#422 port its gates.

use crate::gate::Gate;
use crate::gates::gate_status::GATE_STATUS;
use crate::gates::push_waiver::PUSH_WAIVER;

/// The `PreToolUse` built-ins, in table order.
pub const PRE_TOOL: &[&dyn Gate] = &[];

/// The `PostToolUse` built-ins, in table order: gate status before push waiver.
pub const POST_TOOL: &[&dyn Gate] = &[&GATE_STATUS, &PUSH_WAIVER];
