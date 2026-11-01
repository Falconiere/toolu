//! Worker hosts (`scripts/hosts.ts`): how `herdr agent start` runs each agent
//! CLI, which flags skip its approval prompts, how model and effort are passed,
//! how a relaunch resumes the previous conversation, and how a brief names a
//! skill. Every argument must be shell-safe: herdr types the command into the
//! pane's interactive shell.

use regex::{Regex, RegexBuilder};

/// The four worker hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HostKind {
  /// Claude Code.
  Claude,
  /// Codex.
  Codex,
  /// Cursor Agent.
  Cursor,
  /// Muse Code.
  Muse,
}

/// Every host, in pool order.
pub(crate) const HOST_KINDS: [HostKind; 4] = [
  HostKind::Claude,
  HostKind::Codex,
  HostKind::Cursor,
  HostKind::Muse,
];

/// Usage-limit or rate-limit messages the agent CLIs print when throttled.
/// Muse also says `model limit reached` and `You've used your <plan> quota`.
pub(crate) const HOST_LIMIT: &str = "usage limit|rate[- ]limit(ed)?\\b|hit your (usage )?limit|quota (exceeded|reached)|too many requests|\\b429\\b|limit will reset|try again (at|in) |model limit reached|you've used your [a-z0-9 -]*quota";

impl HostKind {
  /// The herdr `--kind` and record name.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Claude => "claude",
      Self::Codex => "codex",
      Self::Cursor => "cursor",
      Self::Muse => "muse",
    }
  }
}

/// The host a name or alias refers to.
pub(crate) fn parse_host_kind(name: &str) -> Option<HostKind> {
  match name.trim().to_lowercase().as_str() {
    "claude" | "claude-code" => Some(HostKind::Claude),
    "codex" => Some(HostKind::Codex),
    "cursor" | "cursor-agent" => Some(HostKind::Cursor),
    "muse" => Some(HostKind::Muse),
    _ => None,
  }
}

/// The host a name or alias refers to.
///
/// # Errors
/// The name is no host.
pub(crate) fn host_kind(name: &str) -> Result<HostKind, String> {
  parse_host_kind(name)
    .ok_or_else(|| format!("unknown host {name}; use one of claude, codex, cursor, muse"))
}

/// Whether `text` holds a provider-limit message.
pub(crate) fn host_limited(text: &str) -> bool {
  RegexBuilder::new(HOST_LIMIT)
    .case_insensitive(true)
    .build()
    .is_ok_and(|re| re.is_match(text))
}

/// What one `agent start` needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AgentOpts {
  /// The herdr agent name.
  pub key: String,
  /// The model, when routed.
  pub model: Option<String>,
  /// The effort, when routed.
  pub effort: Option<String>,
  /// Unattended (approvals off).
  pub bypass: bool,
  /// Claude's permission mode when attended.
  pub permission_mode: String,
  /// Continue the previous conversation.
  pub resume: bool,
  /// The captured session to resume exactly.
  pub session_id: Option<String>,
  /// The settings file a Claude launch wrote, passed as `--settings`.
  pub settings: Option<String>,
}

fn words(items: &[&str]) -> Vec<String> {
  items.iter().map(|item| (*item).to_owned()).collect()
}

fn approvals(kind: HostKind, opts: &AgentOpts) -> Vec<String> {
  match (kind, opts.bypass) {
    (HostKind::Claude, true) => words(&["--dangerously-skip-permissions"]),
    (HostKind::Claude, false) => vec!["--permission-mode".to_owned(), opts.permission_mode.clone()],
    (HostKind::Codex, true) => words(&["--dangerously-bypass-approvals-and-sandbox"]),
    (HostKind::Codex, false) => words(&[
      "--ask-for-approval",
      "on-request",
      "--sandbox",
      "workspace-write",
    ]),
    (HostKind::Cursor, true) => words(&["--yolo", "--trust", "--approve-mcps"]),
    (HostKind::Cursor, false) => words(&["--trust"]),
    (HostKind::Muse, true) => words(&["--yolo"]),
    (HostKind::Muse, false) => words(&["--approval-mode", "on-request", "--trust-workspace"]),
  }
}

fn model_args(model: &str) -> Vec<String> {
  vec!["--model".to_owned(), model.to_owned()]
}

fn effort_args(kind: HostKind, effort: &str) -> Vec<String> {
  match kind {
    HostKind::Claude => vec!["--effort".to_owned(), effort.to_owned()],
    // Unquoted TOML fails to parse, so codex takes the raw string literally.
    HostKind::Codex => vec!["-c".to_owned(), format!("model_reasoning_effort={effort}")],
    // Cursor encodes effort in the model id.
    HostKind::Cursor => Vec::new(),
    HostKind::Muse => vec!["--reasoning-effort".to_owned(), effort.to_owned()],
  }
}

fn resume_args(kind: HostKind, opts: &AgentOpts) -> (Vec<String>, Vec<String>) {
  if let (true, Some(id)) = (opts.resume, opts.session_id.as_deref()) {
    let lead = match kind {
      HostKind::Codex | HostKind::Muse => vec!["resume".to_owned(), id.to_owned()],
      HostKind::Claude | HostKind::Cursor => vec!["--resume".to_owned(), id.to_owned()],
    };
    return (lead, Vec::new());
  }
  if !opts.resume {
    return (Vec::new(), Vec::new());
  }
  match kind {
    HostKind::Codex | HostKind::Muse => (words(&["resume", "--last"]), Vec::new()),
    HostKind::Claude | HostKind::Cursor => (Vec::new(), words(&["--continue"])),
  }
}

/// The argv after `--` in `herdr agent start` for `kind`.
///
/// # Errors
/// An argument the pane shell would mangle.
pub(crate) fn agent_args(kind: HostKind, opts: &AgentOpts) -> Result<Vec<String>, String> {
  let (lead, tail) = resume_args(kind, opts);
  let mut args = lead;
  if kind == HostKind::Codex {
    args.push("--no-daemon".to_owned());
  }
  args.extend(approvals(kind, opts));
  if kind == HostKind::Claude {
    args.extend(["-n".to_owned(), opts.key.clone()]);
  }
  if let Some(model) = opts.model.as_deref() {
    args.extend(model_args(model));
  }
  if let Some(effort) = opts.effort.as_deref() {
    args.extend(effort_args(kind, effort));
  }
  if let (HostKind::Claude, Some(settings)) = (kind, opts.settings.as_deref()) {
    args.extend(["--settings".to_owned(), settings.to_owned()]);
  }
  args.extend(tail);
  let safe = Regex::new(r"^[A-Za-z0-9_./:=,@%+#-]+$").map_err(|err| err.to_string())?;
  if let Some(bad) = args.iter().find(|arg| !safe.is_match(arg)) {
    return Err(format!(
      "{} arg the pane shell would mangle: {bad}",
      kind.name()
    ));
  }
  Ok(args)
}

/// How a brief tells `kind` to run `plugin:skill`.
pub(crate) fn skill_ref(kind: HostKind, plugin: &str, skill: &str) -> String {
  match kind {
    HostKind::Claude => format!("`/{plugin}:{skill}`"),
    HostKind::Codex => format!("`${plugin}:{skill}`"),
    HostKind::Cursor => format!("the `{plugin}:{skill}` skill"),
    // Muse names plugin skills `plugin:<plugin>:<skill>`.
    HostKind::Muse => format!("the `plugin:{plugin}:{skill}` skill"),
  }
}

#[cfg(test)]
#[path = "tests/hosts_test.rs"]
mod tests;
