//! ts-quality's `check-toolu` `SessionStart` hook: Codex has no plugin-dependency
//! manifest field, so warn when this plugin is installed without the toolu
//! core; silent on hosts that resolve `plugin.json` dependencies themselves.

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::dependencies::{NoticeStyle, codex_dependency_notice};

/// The core plugin ts-quality needs.
const CORE_PLUGIN: &str = "toolu@toolu";

/// The Codex warning when the toolu core is not installed; silent elsewhere.
pub fn check_toolu(env: &Env) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  let notice = codex_dependency_notice(&[CORE_PLUGIN], NoticeStyle::Core, &roots);
  Outcome {
    exit: Exit::Success,
    stdout: notice.map(|line| line.trim_end_matches('\n').to_owned()),
    stderr: None,
  }
}

#[cfg(test)]
#[path = "tests/hooks_test.rs"]
mod tests;
