//! The compiled-in ts-quality rule: the registry [`Rule`] its manifest enables,
//! and the [`QualityRule`] the shared post-edit runner drives for every edited
//! `.ts` or `.tsx` file of a TypeScript project with a package manager.

use std::path::PathBuf;

use toolu_engine::quality::{
  AstGrepScan, EditedFile, QualityFindings, QualityRule, edited_file, run_quality_event,
};
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::config::load::load;
use toolu_runtime::config::quality::{
  QualityLang, quality_flag, quality_threshold, ts_max_file_lines_resolved,
};
use toolu_runtime::git::toplevel;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};
use toolu_state::detect::project::{NodePackageManager, detect_ts, node_package_manager};
use toolu_state::detect::tools::tool_available;

use crate::advisories::{docs, duplication, unhandled_await};
use crate::ast_rules::{error_handling, inline_rules, mock_doubles};
use crate::file::{Limits, Place, TsFile, has_extension};
use crate::layout_rules::{parent_import, test_placement};
use crate::line_rules::{
  catch_toast, component_file_name, confirm_alert, console_log, factories, manual_type_guard,
  mutable_props, raw_radix_import, react_hooks, suppression_comment, type_assertion,
};
use crate::size_rules::{file_too_long, function_too_long};
use crate::type_rules::{duplicate_types, throw_literal};

/// ts-quality's post-edit rule.
pub(crate) struct TsQuality;

/// The rule its `ts-quality@toolu__ts-quality.json` manifest enables.
pub const TS_QUALITY: &dyn Rule = &TsQuality;

/// The manifest spec.
pub(crate) const SPEC: &str = "ts-quality@toolu";
/// The manifest and rule name.
pub(crate) const NAME: &str = "ts-quality";

fn cwd<'a>(ctx: &RuleContext<'a>) -> &'a std::path::Path {
  ctx.cwd.unwrap_or(ctx.project_root)
}

fn package_manager(ctx: &RuleContext<'_>) -> Option<NodePackageManager> {
  node_package_manager(ctx.env, cwd(ctx)).filter(|pm| tool_available(pm.name(), ctx.env))
}

fn limits(ctx: &RuleContext<'_>) -> Limits {
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let config = load(&roots, ctx.cwd);
  let root = toplevel(ctx.env, cwd(ctx));
  let (file_lines, file_source) = ts_max_file_lines_resolved(&config, root.as_deref());
  Limits {
    file_lines,
    file_source,
    fn_lines: quality_threshold(&config, QualityLang::Ts, "maxFnLines", root.as_deref()),
    no_mocks: quality_flag(&config, QualityLang::Ts, "noMocks", true),
  }
}

fn read(file: &EditedFile) -> Result<String, String> {
  std::fs::read(&file.absolute)
    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    .map_err(|error| {
      format!(
        "Cannot read {} for TypeScript quality checks: {error}",
        file.path
      )
    })
}

type Check = fn(&TsFile<'_>) -> Vec<String>;

fn one(found: Option<String>) -> Vec<String> {
  found.into_iter().collect()
}

/// Every rule in the numeric order of the bash fragments they replace; the two
/// structural rules read the scan.
fn errors(f: &TsFile<'_>, scan: &AstGrepScan) -> Vec<String> {
  let before: [Check; 16] = [
    |f| one(parent_import(f)),
    |f| one(type_assertion(f)),
    test_placement,
    |f| one(file_too_long(f)),
    |f| one(function_too_long(f)),
    |f| one(react_hooks(f)),
    |f| one(factories(f)),
    |f| one(manual_type_guard(f)),
    duplicate_types,
    |f| one(component_file_name(f)),
    |f| one(console_log(f)),
    |f| one(suppression_comment(f)),
    |f| one(confirm_alert(f)),
    |f| one(raw_radix_import(f)),
    |f| one(mutable_props(f)),
    |f| one(catch_toast(f)),
  ];
  let mut errors: Vec<String> = before.iter().flat_map(|check| check(f)).collect();
  errors.extend(error_handling(f, scan));
  errors.extend(one(throw_literal(f)));
  errors.extend(mock_doubles(f, scan));
  errors
}

impl QualityRule for TsQuality {
  fn language(&self) -> &'static str {
    "ts"
  }

  fn extensions(&self) -> &[&str] {
    &["ts", "tsx"]
  }

  fn source(&self) -> &'static str {
    "ts-quality-hook"
  }

  fn reason(&self) -> &'static str {
    "Post-edit quality violation(s) detected"
  }

  fn project_enabled(&self, ctx: &RuleContext<'_>) -> bool {
    detect_ts(ctx.env, cwd(ctx)) && package_manager(ctx).is_some()
  }

  fn skip_linked_worktrees(&self) -> bool {
    true
  }

  fn ast_rule_dirs(&self) -> Vec<PathBuf> {
    Vec::new()
  }

  fn ast_rules_inline(&self, ctx: &RuleContext<'_>) -> String {
    inline_rules(ctx.host)
  }

  fn check(&self, file: &EditedFile, ctx: &RuleContext<'_>, scan: &AstGrepScan) -> QualityFindings {
    let Some(pm) = package_manager(ctx) else {
      return QualityFindings::default();
    };
    let text = match read(file) {
      Ok(text) => text,
      Err(error) => {
        return QualityFindings {
          errors: vec![error],
          advisories: Vec::new(),
        };
      }
    };
    let f = TsFile::new(file, &text, Place::of(ctx), limits(ctx), pm);
    let errors = errors(&f, scan);
    let duplicated = if errors.is_empty() {
      duplication(&f)
    } else {
      String::new()
    };
    QualityFindings {
      errors,
      advisories: vec![duplicated, docs(&f), unhandled_await(&f)],
    }
  }
}

impl Rule for TsQuality {
  fn spec(&self) -> &str {
    SPEC
  }

  fn name(&self) -> &str {
    NAME
  }

  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPost
  }

  fn applies(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> bool {
    edited_file(event, ctx).is_some_and(|file| has_extension(&file.path, &["ts", "tsx"]))
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
    let outcome = run_quality_event(event, ctx, &[self]);
    warnings.extend(outcome.warnings);
    outcome
      .decisions
      .into_iter()
      .next()
      .unwrap_or(Decision::Allow)
  }
}

#[cfg(test)]
#[path = "tests/rule_test.rs"]
mod tests;
