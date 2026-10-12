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
  /// `OpenCode`.
  Opencode,
}

/// Every host, in pool order.
pub(crate) const HOST_KINDS: [HostKind; 4] = [
  HostKind::Claude,
  HostKind::Codex,
  HostKind::Cursor,
  HostKind::Opencode,
];

/// Usage-limit or rate-limit messages the agent CLIs print when throttled.
pub(crate) const HOST_LIMIT: &str = "usage limit|rate[- ]limit(ed)?\\b|hit your (usage )?limit|quota (exceeded|reached)|too many requests|\\b429\\b|limit will reset|try again (at|in) ";

/// The generated `OpenCode` skill name of each `plugin:skill` a brief names.
pub(crate) const OPENCODE_SKILL_IDS: [(&str, &str); 3] = [
  ("delivery-flow:delivery-flow", "delivery-flow-delivery-flow"),
  ("pr-babysit:babysit", "pr-babysit-babysit-73c340c6"),
  ("toolu:debug", "toolu-debug"),
];

impl HostKind {
  /// The herdr `--kind` and record name.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Self::Claude => "claude",
      Self::Codex => "codex",
      Self::Cursor => "cursor",
      Self::Opencode => "opencode",
    }
  }
}

/// The host a name or alias refers to.
pub(crate) fn parse_host_kind(name: &str) -> Option<HostKind> {
  match name.trim().to_lowercase().as_str() {
    "claude" | "claude-code" => Some(HostKind::Claude),
    "codex" => Some(HostKind::Codex),
    "cursor" | "cursor-agent" => Some(HostKind::Cursor),
    "opencode" => Some(HostKind::Opencode),
    _ => None,
  }
}

/// The host a name or alias refers to.
///
/// # Errors
/// The name is no host.
pub(crate) fn host_kind(name: &str) -> Result<HostKind, String> {
  parse_host_kind(name)
    .ok_or_else(|| format!("unknown host {name}; use one of claude, codex, cursor, opencode"))
}

/// Whether `text` holds a provider-limit message.
pub(crate) fn host_limited(text: &str) -> bool {
  RegexBuilder::new(HOST_LIMIT)
    .case_insensitive(true)
    .build()
    .is_ok_and(|re| re.is_match(text))
}

/// The TUI takes `provider/model` and has no variant flag.
///
/// # Errors
/// The model is not `provider/model`.
pub(crate) fn opencode_model_args(model: &str) -> Result<Vec<String>, String> {
  let valid = Regex::new(r"^[^/\s#]+/[^\s#]+$").is_ok_and(|re| re.is_match(model));
  if valid {
    Ok(vec!["--model".to_owned(), model.to_owned()])
  } else {
    Err(format!(
      "OpenCode model must be provider/model, got {model}"
    ))
  }
}

/// Why `opencode --version` output is not a 1.x release, or `None` when it is.
pub(crate) fn opencode_version_problem(output: &str) -> Option<String> {
  let version = Regex::new(r"\d+\.\d+\.\d+")
    .ok()
    .and_then(|re| re.find(output).map(|found| found.as_str().to_owned()));
  if version
    .as_deref()
    .is_some_and(|version| version.starts_with("1."))
  {
    return None;
  }
  let seen = version.map_or_else(
    || "printed no version".to_owned(),
    |version| format!("reports \"{version}\""),
  );
  Some(format!(
    "opencode --version {seen}; toolu's OpenCode plugin targets opencode-ai 1.x. Put a 1.x opencode first on PATH, or route this issue to another host."
  ))
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
    (HostKind::Opencode, true) => words(&["--auto"]),
    (HostKind::Opencode, false) => Vec::new(),
  }
}

fn model_args(kind: HostKind, model: &str) -> Result<Vec<String>, String> {
  match kind {
    HostKind::Opencode => opencode_model_args(model),
    HostKind::Claude | HostKind::Codex | HostKind::Cursor => {
      Ok(vec!["--model".to_owned(), model.to_owned()])
    }
  }
}

fn effort_args(kind: HostKind, effort: &str) -> Vec<String> {
  match kind {
    HostKind::Claude => vec!["--effort".to_owned(), effort.to_owned()],
    // Unquoted TOML fails to parse, so codex takes the raw string literally.
    HostKind::Codex => vec!["-c".to_owned(), format!("model_reasoning_effort={effort}")],
    // Cursor encodes effort in the model id; OpenCode has no effort flag.
    HostKind::Cursor | HostKind::Opencode => Vec::new(),
  }
}

fn resume_args(kind: HostKind, opts: &AgentOpts) -> (Vec<String>, Vec<String>) {
  if let (true, Some(id)) = (opts.resume, opts.session_id.as_deref()) {
    let lead = match kind {
      HostKind::Codex => vec!["resume".to_owned(), id.to_owned()],
      HostKind::Opencode => vec!["--session".to_owned(), id.to_owned()],
      HostKind::Claude | HostKind::Cursor => vec!["--resume".to_owned(), id.to_owned()],
    };
    return (lead, Vec::new());
  }
  if !opts.resume {
    return (Vec::new(), Vec::new());
  }
  match kind {
    HostKind::Codex => (words(&["resume", "--last"]), Vec::new()),
    HostKind::Claude | HostKind::Cursor | HostKind::Opencode => {
      (Vec::new(), words(&["--continue"]))
    }
  }
}

/// The argv after `--` in `herdr agent start` for `kind`.
///
/// # Errors
/// An `OpenCode` model that is not `provider/model`, or an argument the pane
/// shell would mangle.
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
    args.extend(model_args(kind, model)?);
  }
  if let Some(effort) = opts.effort.as_deref() {
    args.extend(effort_args(kind, effort));
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
///
/// # Errors
/// `OpenCode` has no generated skill for it.
pub(crate) fn skill_ref(kind: HostKind, plugin: &str, skill: &str) -> Result<String, String> {
  Ok(match kind {
    HostKind::Claude => format!("`/{plugin}:{skill}`"),
    HostKind::Codex => format!("`${plugin}:{skill}`"),
    HostKind::Cursor => format!("the `{plugin}:{skill}` skill"),
    HostKind::Opencode => {
      let wanted = format!("{plugin}:{skill}");
      let id = OPENCODE_SKILL_IDS
        .iter()
        .find(|(name, _)| *name == wanted)
        .map(|(_, id)| *id)
        .ok_or_else(|| format!("no OpenCode skill id for {wanted}"))?;
      format!("`skill({{ name: \"{id}\" }})`")
    }
  })
}

#[cfg(test)]
#[path = "tests/hosts_test.rs"]
mod tests;
