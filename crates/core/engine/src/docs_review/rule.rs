//! Native pre-push rule and session registration.
use super::{
  assess,
  push::{nothing_to_review, reviewed_head},
};
use crate::detect::possible_push;
use crate::gates::{command_analysis, decided, pre_mode, push_target::push_target_in};
use crate::quality::publish;
use toolu_protocol::{decision::Decision, host::Host, normalized::NormalizedEvent};
use toolu_runtime::{
  cli::Outcome,
  config::gate_mode::GateMode,
  env::Env,
  host::roots::Roots,
  registry::{
    ModuleKind, RegistryEvent, event_dir, file_name,
    manifest::{ModuleManifest, read_manifest},
    rule::{Rule, RuleContext},
  },
};

const SPEC: &str = "documentation-gate@toolu";
const NAME: &str = "docs-review";

/// Whether the valid pre-tool manifest enables this rule.
pub fn active(roots: &Roots) -> bool {
  crate::registry::gate::plugin_active(SPEC, roots)
    && file_name(SPEC, NAME, ModuleKind::Manifest).is_ok_and(|file| {
      read_manifest(
        &event_dir(roots, RegistryEvent::ToolPre).join(file),
        RegistryEvent::ToolPre,
      )
      .is_ok_and(|m| m.spec == SPEC && m.name == NAME && m.matcher == "*")
    })
}

/// Register the native pre-tool rule for this host; silent unless it fails.
pub fn register(env: &Env, host: Host) -> Outcome {
  let roots = Roots::new(env.clone(), Some(host));
  let manifest = ModuleManifest {
    version: 1,
    spec: SPEC.into(),
    name: NAME.into(),
    event: RegistryEvent::ToolPre,
    matcher: "*".into(),
  };
  Outcome::notice(publish(env, &roots, &manifest).err().map(|error| {
    let sync = roots
      .invocation("documentation-gate", "sync")
      .unwrap_or_default();
    format!(
      "documentation-gate registration failed: {error}. Run {sync} after fixing registration."
    )
  }))
}

/// The compiled-in documentation review rule.
struct DocsReview;
/// The rule singleton registered by the hub.
pub static RULE: &dyn Rule = &DocsReview;
impl Rule for DocsReview {
  fn spec(&self) -> &str {
    SPEC
  }
  fn name(&self) -> &str {
    NAME
  }
  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPre
  }
  fn applies(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> bool {
    matches!(event, NormalizedEvent::ShellPre { .. })
  }
  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    evaluate(event, ctx, &mut Vec::new())
  }
  fn run_warning(
    &self,
    event: &NormalizedEvent,
    ctx: &RuleContext<'_>,
    warnings: &mut Vec<String>,
  ) -> Decision {
    evaluate(event, ctx, warnings)
  }
}

/// The target repository's roots and effective `docsSync` mode.
fn target_mode(
  ctx: &RuleContext<'_>,
  root: &std::path::Path,
  warnings: &mut Vec<String>,
) -> (Roots, GateMode) {
  let env = ctx
    .env
    .clone()
    .with("TOOLU_PROJECT_DIR", &root.to_string_lossy());
  let roots = Roots::new(env, Some(ctx.host));
  let config = toolu_runtime::config::load::load(&roots, Some(root));
  let mode = pre_mode(&config, "docsSync", ctx, true);
  warnings.extend(config.take_warnings());
  (roots, mode)
}

fn evaluate(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  warnings: &mut Vec<String>,
) -> Decision {
  if !matches!(event, NormalizedEvent::ShellPre { .. }) {
    return Decision::Allow;
  }
  let analysis = command_analysis(ctx);
  if let Some(dir) = possible_push(&analysis, ctx.cwd.unwrap_or(ctx.project_root), ctx.env) {
    let root = toolu_runtime::git::toplevel(ctx.env, &dir).unwrap_or(dir);
    let (_, mode) = target_mode(ctx, &root, warnings);
    return decided(
      mode,
      "documentation-gate: this command may push through a dynamic git subcommand, a Git alias or a plumbing command; run a static git push <remote> HEAD:<branch> on its own".to_owned(),
    )
    .unwrap_or(Decision::Allow);
  }
  let Some(target) = push_target_in(&analysis, ctx) else {
    return Decision::Allow;
  };
  let (roots, mode) = target_mode(ctx, &target.root, warnings);
  if mode == GateMode::Off || nothing_to_review(&analysis, &roots, &target.root) {
    return Decision::Allow;
  }
  match reviewed_head(&analysis, &target.root, ctx.env)
    .and_then(|()| assess(&roots, &target.root, &target.branch))
  {
    Ok(()) => Decision::Allow,
    // The fixed prefix makes the reason nonempty, so conversion cannot fail.
    Err(error) => decided(
      mode,
      format!("documentation-gate: current documentation review required: {error}"),
    )
    .unwrap_or(Decision::Allow),
  }
}

#[cfg(test)]
#[path = "tests/rule_test.rs"]
mod tests;
