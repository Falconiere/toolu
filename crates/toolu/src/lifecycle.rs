//! toolu's session lifecycle hooks (#424): `session-start`, `user-prompt-submit`,
//! and `pre-compact`. The Markdown under `plugins/toolu/hooks/docs` stays the
//! Session Protocol. `crates/cli` dispatches these names here.

pub(crate) mod dependencies;
pub(crate) mod diagnostics;
pub(crate) mod docs;
pub(crate) mod housekeeping;
pub(crate) mod mandates;
pub(crate) mod notices;
pub mod pre_compact;
pub(crate) mod presence;
pub(crate) mod project;
pub(crate) mod prompt;
pub mod prompt_submit;
pub mod session;
pub(crate) mod text;
