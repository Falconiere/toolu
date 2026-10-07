//! What a dispatch did with each module it reached, for tests and diagnostics:
//! `ModuleOutcome` in `registry-run.ts`, extended to built-ins and rules.

/// One module reached by a walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
  /// The built-in's name or the registry file name.
  pub module: String,
  /// What kind of module it is.
  pub kind: StepKind,
  /// What happened to it.
  pub status: StepStatus,
}

/// What kind of module a step ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
  /// A built-in gate.
  Builtin,
  /// A compiled-in rule enabled by a manifest.
  Rule,
  /// A `.sh` module.
  Executable,
  /// A `.js` module, run through the Bun bridge.
  Esm,
}

/// What happened to a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
  /// It returned a decision.
  Decided,
  /// The `.sh` module exited with this status.
  Exited(i32),
  /// It failed; its output was skipped.
  Failed(String),
  /// It did not run.
  Skipped(Skip),
}

/// Why a module did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
  /// Its plugin is not installed or not selected.
  Inactive,
  /// A newer module of its spec stands in for it.
  Shadowed,
  /// Its manifest's matcher, or its rule's `applies`, does not fit the event.
  NotMatching,
  /// It is a `.js` module and Bun could not be found or started.
  NoBun,
}
