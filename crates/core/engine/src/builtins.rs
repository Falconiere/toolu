//! The built-in gate tables of `toolu hook pre-tools` and `toolu hook post-tools`
//! (`NATIVE_MODULES` in `plugins/toolu/hooks/src/pre-tools/builtins.ts`). Empty
//! until the gates are ported (#419–#423), which append to them in order.

use crate::gate::Gate;

/// The `PreToolUse` built-ins, in table order.
pub const PRE_TOOL: &[&dyn Gate] = &[];

/// The `PostToolUse` built-ins, in table order.
pub const POST_TOOL: &[&dyn Gate] = &[];
