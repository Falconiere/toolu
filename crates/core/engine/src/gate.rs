//! The built-in gate contract (`ToolModule` in `dispatch-context.ts`): a native
//! gate run in process before the registry, in its table's order.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::rule::RuleContext;

/// A built-in gate.
pub trait Gate: Sync {
  /// The name stderr lines use.
  fn name(&self) -> &str;
  /// Decides one event. `Err` is a thrown error: the gate counts as exit 1, so
  /// only `toolu-dispatch: module <name> exited 1; output skipped` is printed.
  ///
  /// # Errors
  /// Why the gate could not decide; the dispatcher reports it as an exit 1.
  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String>;
  /// [`Gate::run`], also collecting the warnings TypeScript prints to stderr
  /// as it goes (`stderrWarn`). The walk prints each on its own line before
  /// folding the decision. By default, `run` with no warnings.
  ///
  /// # Errors
  /// As [`Gate::run`].
  fn run_warning(
    &self,
    event: &NormalizedEvent,
    ctx: &RuleContext<'_>,
    _warnings: &mut Vec<String>,
  ) -> Result<Decision, String> {
    self.run(event, ctx)
  }
}
