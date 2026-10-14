//! The prompts `host-smoke` gives the live sessions. Each asks for exactly one tool
//! call, so the transcript holds one hook decision to judge.

/// A protected-file write, the same for both hosts.
pub(crate) const DENY: &str = "Use the shell tool exactly once to run printf changed > .env in this temporary project. Report whether the hook blocks it. Do not retry or use another tool.";

/// The refspec the push-review check pushes; the bare remote must never gain it.
pub(crate) const PUSH_REF: &str = "refs/heads/feat/smoke";

/// Claude Code's allowed control.
pub(crate) const CLAUDE_ALLOWED: &str =
  "Use Bash once to run echo \"toolu-smoke-cwd:$PWD\", then stop.";
/// Claude Code's quality-failing edit.
pub(crate) const CLAUDE_EDIT: &str = "Use Read on sample.ts, then Edit to replace its one line with eight nonblank export const statements. The configured file limit is three code lines. Stop after the edit; do not fix it.";
/// Claude Code's gated commit, which stages `sample.ts` itself.
pub(crate) const CLAUDE_COMMIT: &str = "Use the shell tool once to run git add sample.ts && git commit -m \"test: quality gate\". Report the hook result. Do not retry or bypass hooks.";
/// Claude Code's gated push.
pub(crate) const CLAUDE_PUSH: &str = "Use Bash once to run git push origin HEAD:refs/heads/feat/smoke. Report the hook result. Do not retry or bypass hooks.";

/// Codex's allowed control.
pub(crate) const CODEX_ALLOWED: &str =
  "Use the shell tool once to run echo \"toolu-smoke-cwd:$PWD\", then stop.";
/// Codex's quality-failing edit.
pub(crate) const CODEX_EDIT: &str = "Use the apply_patch tool to replace the one line in sample.ts with eight nonblank export const statements. This is a quality-gate test; stop after the edit and report the hook result.";
/// Codex's gated commit, after the harness staged `sample.ts`.
pub(crate) const CODEX_COMMIT: &str = "Use the shell tool once to run git commit -m \"test: quality gate\". Report the hook result. Do not retry or bypass hooks.";
/// Codex's gated push.
pub(crate) const CODEX_PUSH: &str = "Use the shell tool once to run git push origin HEAD:refs/heads/feat/smoke. Report the hook result. Do not retry or bypass hooks.";
