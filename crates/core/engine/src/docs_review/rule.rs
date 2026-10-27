//! Native pre-push rule and session registration.
use super::{
  assess,
  push::{dynamic_push, reviewed_head},
};
use crate::gates::{command_analysis, decided, gate_config, pre_mode, push_target::push_target};
use toolu_protocol::{decision::Decision, host::Host, normalized::NormalizedEvent};
use toolu_runtime::{
  cli::Outcome,
  env::Env,
  host::roots::Roots,
  registry::{
    RegistryEvent, event_dir,
    manifest::{ModuleManifest, read_manifest},
    rule::{Rule, RuleContext},
  },
};

const SPEC: &str = "documentation-gate@toolu";
const NAME: &str = "docs-review";
const FILE: &str = "documentation-gate@toolu__docs-review.json";

/// Whether the valid pre-tool manifest enables this rule.
pub fn active(roots: &Roots) -> bool {
  crate::registry::gate::plugin_active(SPEC, roots)
    && read_manifest(
      &event_dir(roots, RegistryEvent::ToolPre).join(FILE),
      RegistryEvent::ToolPre,
    )
    .is_ok_and(|m| m.spec == SPEC && m.name == NAME && m.matcher == "*")
}

/// Register the native pre-tool rule for this host.
pub fn register(env: &Env, host: Host) -> Outcome {
  let roots = Roots::new(env.clone(), Some(host));
  let manifest = ModuleManifest {
    version: 1,
    spec: SPEC.into(),
    name: NAME.into(),
    event: RegistryEvent::ToolPre,
    matcher: "*".into(),
  };
  let target = event_dir(&roots, RegistryEvent::ToolPre).join(FILE);
  let result = serde_json::to_string(&manifest)
    .map_err(|e| e.to_string())
    .and_then(|body| {
      let body = format!("{body}\n");
      if std::fs::read(&target).is_ok_and(|old| old == body.as_bytes()) {
        return Ok(());
      }
      toolu_runtime::atomic::replace_atomic(&target, &body).map_err(|e| e.to_string())
    })
    .and_then(|()| startup_report(env, &target));
  let message = match result {
    Ok(()) => "Documentation review is active. Run documentation-gate:sync before pushing changes."
      .to_owned(),
    Err(error) => format!(
      "documentation-gate registration failed: {error}. Run documentation-gate:sync after fixing registration."
    ),
  };
  Outcome::notice(Some(message))
}

fn startup_report(env: &Env, target: &std::path::Path) -> Result<(), String> {
  use std::io::Write as _;
  let Some(path) = env.get("TOOLU_STARTUP_REPORT") else {
    return Ok(());
  };
  let record = serde_json::json!({"kind":"native-registry", "spec":SPEC, "name":NAME, "event":RegistryEvent::ToolPre, "matcher":"*", "target":target});
  let mut file = std::fs::OpenOptions::new()
    .append(true)
    .open(path)
    .map_err(|e| e.to_string())?;
  writeln!(file, "{record}").map_err(|e| e.to_string())
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

fn evaluate(
  event: &NormalizedEvent,
  ctx: &RuleContext<'_>,
  warnings: &mut Vec<String>,
) -> Decision {
  let analysis = command_analysis(ctx);
  if dynamic_push(&analysis) {
    let config = gate_config(ctx);
    let mode = pre_mode(&config, "docsSync", ctx, true);
    warnings.extend(config.take_warnings());
    return decided(
      mode,
      "documentation-gate: use a static git push <remote> HEAD:<branch>".to_owned(),
    )
    .unwrap_or(Decision::Allow);
  }
  let Some(target) = push_target(event, ctx) else {
    return Decision::Allow;
  };
  let env = ctx
    .env
    .clone()
    .with("TOOLU_PROJECT_DIR", &target.root.to_string_lossy());
  let roots = Roots::new(env, Some(ctx.host));
  let config = toolu_runtime::config::load::load(&roots, Some(&target.root));
  let mode = pre_mode(&config, "docsSync", ctx, true);
  warnings.extend(config.take_warnings());
  let assessment = if mode == toolu_runtime::config::gate_mode::GateMode::Off {
    Ok(())
  } else if !reviewed_head(&analysis, &target.root, ctx.env) {
    Err("push one reviewed HEAD using git push <remote> HEAD:<branch>".to_owned())
  } else {
    assess(&roots, &target.root, &target.branch)
  };
  match assessment {
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
