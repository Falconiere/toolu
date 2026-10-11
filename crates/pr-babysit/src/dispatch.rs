//! `dispatch-fix start|wait|cleanup`: run a route plan's groups one after
//! another, each a fixer agent in a herdr pane or a detached OpenCode process
//! in the slot's own worktree, and settle each from its report.

pub(crate) mod brief;
pub(crate) mod herdr;
pub(crate) mod names;
pub(crate) mod opencode;
pub(crate) mod outcome;
pub(crate) mod procs;
