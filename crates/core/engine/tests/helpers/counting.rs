//! A compiled-in rule for tests that counts its `applies` and `run` calls, and
//! the hook payloads the rule tests send.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};

/// A rule that counts its `applies` and `run` calls and advises its name.
pub(crate) struct Counting {
  spec: &'static str,
  name: &'static str,
  event: RegistryEvent,
  /// Only paths with this extension apply; every event when `None`.
  pub(crate) extension: Option<&'static str>,
  checks: AtomicUsize,
  runs: AtomicUsize,
}

impl Counting {
  pub(crate) fn new(spec: &'static str, name: &'static str, event: RegistryEvent) -> Counting {
    Counting {
      spec,
      name,
      event,
      extension: None,
      checks: AtomicUsize::new(0),
      runs: AtomicUsize::new(0),
    }
  }

  pub(crate) fn calls(&self) -> (usize, usize) {
    (
      self.checks.load(Ordering::SeqCst),
      self.runs.load(Ordering::SeqCst),
    )
  }
}

impl Rule for Counting {
  fn spec(&self) -> &str {
    self.spec
  }

  fn name(&self) -> &str {
    self.name
  }

  fn event(&self) -> RegistryEvent {
    self.event
  }

  fn applies(&self, _event: &NormalizedEvent, ctx: &RuleContext<'_>) -> bool {
    self.checks.fetch_add(1, Ordering::SeqCst);
    let path = ctx
      .raw
      .get("tool_input")
      .and_then(|input| input.get("file_path")?.as_str());
    self
      .extension
      .is_none_or(|ext| path.is_some_and(|path| Path::new(path).extension() == Some(ext.as_ref())))
  }

  fn run(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Decision {
    self.runs.fetch_add(1, Ordering::SeqCst);
    let tool = event
      .tool()
      .map(|tool| tool.name.as_str().to_owned())
      .unwrap_or_default();
    Text::new(format!("{} saw {tool}", self.name))
      .map_or(Decision::Allow, |message| Decision::Advisory { message })
  }
}

pub(crate) fn payload(tool: &str, input: &str) -> String {
  format!(r#"{{"tool_name":"{tool}","tool_input":{input},"session_id":"s1"}}"#)
}
