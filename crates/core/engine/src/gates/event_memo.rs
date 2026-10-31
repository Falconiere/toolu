//! One hook walk's shell analysis and loaded gate config.
//!
//! [`EventMemoGuard`] is entered for each `walk_with_view`. Dropping it drops
//! the memo, so the next event or edit path cannot reuse it.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use toolu_protocol::host::Host;
use toolu_runtime::config::load::{ConfigFiles, LoadedConfig, config_files, load};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::RuleContext;
use toolu_shell::analysis::ShellAnalysis;

struct Memo {
  command: Option<String>,
  analysis: Option<ShellAnalysis>,
  config_key: Option<ConfigKey>,
  config: Option<LoadedConfig>,
}

impl Memo {
  fn fresh() -> Self {
    Self {
      command: None,
      analysis: None,
      config_key: None,
      config: None,
    }
  }
}

#[derive(PartialEq, Eq)]
struct ConfigKey {
  host: Host,
  cwd: Option<PathBuf>,
  files: ConfigFiles,
}

thread_local! {
  static MEMO: RefCell<Option<Memo>> = const { RefCell::new(None) };
  static PARSES: Cell<usize> = const { Cell::new(0) };
  static LOADS: Cell<usize> = const { Cell::new(0) };
}

/// Memo scope for one path walk. The analysis and config live until this drops.
#[must_use = "the analysis and config live until this guard drops"]
pub(crate) struct EventMemoGuard {
  previous: Option<Memo>,
}

impl EventMemoGuard {
  /// Start a fresh memo for this walk, keeping an outer walk's memo to restore.
  pub(crate) fn enter() -> Self {
    let previous = MEMO.with(|slot| slot.borrow_mut().replace(Memo::fresh()));
    Self { previous }
  }
}

impl Drop for EventMemoGuard {
  fn drop(&mut self) {
    let previous = self.previous.take();
    MEMO.with(|slot| *slot.borrow_mut() = previous);
  }
}

/// The walk's analysis of `command`, parsed on the first miss.
pub(super) fn memoized_analysis(command: &str) -> ShellAnalysis {
  if let Some(hit) = cached_analysis(command) {
    return hit;
  }
  note_parse();
  let analysis = toolu_shell::analyze(command);
  store_analysis(command, &analysis);
  analysis
}

fn cached_analysis(command: &str) -> Option<ShellAnalysis> {
  MEMO.with(|slot| {
    let memo = slot.borrow();
    let memo = memo.as_ref()?;
    if memo.command.as_deref() != Some(command) {
      return None;
    }
    memo.analysis.clone()
  })
}

fn store_analysis(command: &str, analysis: &ShellAnalysis) {
  MEMO.with(|slot| {
    let mut slot = slot.borrow_mut();
    let Some(memo) = slot.as_mut() else {
      return;
    };
    memo.command = Some(command.to_owned());
    memo.analysis = Some(analysis.clone());
  });
}

/// The walk's gate config for `ctx`, loaded on the first miss.
pub(super) fn memoized_config(ctx: &RuleContext<'_>) -> LoadedConfig {
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let key = ConfigKey {
    host: ctx.host,
    cwd: ctx.cwd.map(Path::to_path_buf),
    files: config_files(&roots, ctx.cwd),
  };
  if let Some(hit) = cached_config(&key) {
    return hit;
  }
  note_load();
  let loaded = load(&roots, ctx.cwd);
  store_config(key, loaded.duplicate());
  loaded
}

fn cached_config(key: &ConfigKey) -> Option<LoadedConfig> {
  MEMO.with(|slot| {
    let memo = slot.borrow();
    let memo = memo.as_ref()?;
    if memo.config_key.as_ref() != Some(key) {
      return None;
    }
    memo.config.as_ref().map(LoadedConfig::duplicate)
  })
}

fn store_config(key: ConfigKey, config: LoadedConfig) {
  MEMO.with(|slot| {
    let mut slot = slot.borrow_mut();
    let Some(memo) = slot.as_mut() else {
      return;
    };
    memo.config_key = Some(key);
    memo.config = Some(config);
  });
}

fn note_parse() {
  PARSES.with(|count| count.set(count.get() + 1));
}

fn note_load() {
  LOADS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
#[path = "tests/event_memo_test.rs"]
mod tests;
