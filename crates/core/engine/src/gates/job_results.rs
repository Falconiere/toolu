//! Report finished detached jobs after a shell or edit tool call.

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::registry::rule::RuleContext;

use crate::gate::Gate;
use crate::resources::job_results::take_notice;

/// The post-tool detached-result gate.
pub(crate) struct JobResults;

/// Its singleton.
pub(crate) static JOB_RESULTS: JobResults = JobResults;

impl Gate for JobResults {
  fn name(&self) -> &'static str {
    "job-results"
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Result<Decision, String> {
    let NormalizedEvent::ToolPost { tool, .. } = event else {
      return Ok(Decision::Allow);
    };
    if !matches!(
      tool.name.as_str(),
      "Bash" | "Shell" | "Edit" | "Write" | "MultiEdit" | "apply_patch"
    ) {
      return Ok(Decision::Allow);
    }
    let Some(message) = take_notice(ctx.project_root)? else {
      return Ok(Decision::Allow);
    };
    let message = Text::new(message).map_err(|err| err.to_string())?;
    Ok(Decision::Advisory { message })
  }
}

#[cfg(test)]
#[path = "tests/job_results_test.rs"]
mod tests;
