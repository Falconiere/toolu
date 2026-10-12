//! A [`QualityRule`] as the registry [`Rule`] its plugin's manifest enables:
//! one post-tool rule that applies to an edited file with one of the quality
//! rule's extensions and runs the shared flow for that rule alone.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};

use super::{QualityRule, edited_file, run_quality_event};

/// The registry rule of one compiled-in quality rule.
pub struct ManifestRule<R> {
  /// The manifest spec, such as `ts-quality@toolu`.
  pub spec: &'static str,
  /// The manifest and rule name.
  pub name: &'static str,
  /// The quality rule it runs.
  pub rule: R,
}

impl<R: QualityRule> Rule for ManifestRule<R> {
  fn spec(&self) -> &str {
    self.spec
  }

  fn name(&self) -> &str {
    self.name
  }

  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPost
  }

  fn applies(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> bool {
    edited_file(event, ctx).is_some_and(|file| {
      file
        .absolute
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| self.rule.extensions().contains(&extension))
    })
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    self.run_warning(event, ctx, &mut Vec::new())
  }

  fn run_warning(
    &self,
    event: &NormalizedEvent,
    ctx: &RuleContext<'_>,
    warnings: &mut Vec<String>,
  ) -> Decision {
    let outcome = run_quality_event(event, ctx, &[&self.rule]);
    warnings.extend(outcome.warnings);
    outcome
      .decisions
      .into_iter()
      .next()
      .unwrap_or(Decision::Allow)
  }
}

#[cfg(test)]
#[path = "tests/manifest_rule_test.rs"]
mod tests;
