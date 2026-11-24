//! The prompts `host-smoke` gives the live sessions. Each asks for exactly one tool
//! call, so the transcript holds one hook decision to judge.

/// A protected-file write, the same for both hosts.
pub(crate) const DENY: &str = "Use the shell tool exactly once to run printf changed > .env in this temporary project. Report whether the hook blocks it. Do not retry or use another tool.";

/// The refspec the push-review check pushes; the bare remote must never gain it.
pub(crate) const PUSH_REF: &str = "refs/heads/feat/smoke";

/// The failing quality command, identical on every host: the project's wrapper
/// script exits 1, so the gate-status channel records the gate red.
pub(crate) const FAILING: &str = "Use the shell tool exactly once to run `sh tools/smoke/test.sh` and report its result. It is expected to fail. Do not retry, fix it, or use another tool.";

/// Claude Code's allowed control: it prints the project's nonce through a relative path.
pub(crate) const CLAUDE_ALLOWED: &str =
  "Use Bash once to run exactly `cat .toolu-smoke-nonce`, then stop.";
/// Claude Code's commit under a red quality gate, which must land.
pub(crate) const CLAUDE_COMMIT: &str = "Use Bash once to run git commit --allow-empty -m \"chore(smoke): verify commit\" -m \"Intent: verify delivery gate cadence\". Use that exact command without attribution. Report the hook result. Do not retry or bypass hooks.";
/// Claude Code's gated push.
pub(crate) const CLAUDE_PUSH: &str = "Use Bash once to run git push origin HEAD:refs/heads/feat/smoke. Report the hook result. Do not retry or bypass hooks.";

/// Codex's allowed control.
pub(crate) const CODEX_ALLOWED: &str =
  "Use the shell tool once to run exactly `cat .toolu-smoke-nonce`, then stop.";
/// The commit under a red quality gate for the shell-tool hosts.
pub(crate) const CODEX_COMMIT: &str = "Use the shell tool once to run git commit --allow-empty -m \"chore(smoke): verify commit\" -m \"Intent: verify delivery gate cadence\". Use that exact command without attribution. Report the hook result. Do not retry or bypass hooks.";
/// The push blocked by the quality gate and push review, shell-tool hosts.
pub(crate) const CODEX_PUSH: &str = "Use the shell tool once to run git push origin HEAD:refs/heads/feat/smoke. Report the hook result. Do not retry or bypass hooks.";
