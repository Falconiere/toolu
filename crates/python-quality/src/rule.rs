//! The compiled-in python-quality rule: the registry [`Rule`] its manifest
//! enables, and the [`QualityRule`] the shared post-edit runner drives for every
//! edited `.py` file of a Python project with `python3` on `PATH`. Violations
//! fail the file's quality-gate entry; the docs advisory only informs.

use std::path::{Path, PathBuf};

use toolu_engine::quality::{AstGrepScan, EditedFile, ManifestRule, QualityFindings, QualityRule};
use toolu_runtime::config::load::load;
use toolu_runtime::config::quality::{QualityLang, quality_flag, quality_threshold};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::{Rule, RuleContext};
use toolu_state::detect::project::detect_python;
use toolu_state::detect::tools::tool_available;

use crate::docs::docs;
use crate::file::{Limits, PyFile, base_name, is_python_test_name};
use crate::layout_rules::test_layout;
use crate::no_mocks::no_mocks;
use crate::size_rules::{file_too_long, function_too_long};
use crate::suppression::suppression;

/// python-quality's post-edit rule.
pub(crate) struct PythonQuality;

/// The rule its `python-quality@toolu__python-quality.json` manifest enables.
pub const PYTHON_QUALITY: &dyn Rule = &ManifestRule {
  spec: SPEC,
  name: NAME,
  rule: PythonQuality,
};

/// The manifest spec.
pub(crate) const SPEC: &str = "python-quality@toolu";
/// The manifest and rule name.
pub(crate) const NAME: &str = "python-quality";

fn cwd<'a>(ctx: &RuleContext<'a>) -> &'a Path {
  ctx.cwd.unwrap_or(ctx.project_root)
}

fn limits(ctx: &RuleContext<'_>) -> Limits {
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let config = load(&roots, ctx.cwd);
  let threshold = |key| quality_threshold(&config, QualityLang::Python, key, None);
  Limits {
    file_lines: threshold("maxFileLines"),
    fn_lines: threshold("maxFnLines"),
    no_mocks: quality_flag(&config, QualityLang::Python, "noMocks", true),
  }
}

fn read(file: &EditedFile) -> Result<String, String> {
  std::fs::read(&file.absolute)
    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    .map_err(|error| {
      format!(
        "Cannot read {} for Python quality checks: {error}",
        file.path
      )
    })
}

/// Every rule in the numeric order of the bash fragments they replace.
fn errors(f: &PyFile<'_>, ctx: &RuleContext<'_>) -> Vec<String> {
  let is_test = is_python_test_name(base_name(&f.file.path));
  let mut errors: Vec<String> = file_too_long(f).into_iter().collect();
  errors.extend(test_layout(f));
  errors.extend(suppression(f));
  errors.extend(function_too_long(f));
  errors.extend(no_mocks(f, ctx, is_test));
  errors
}

/// The blocking errors, then the docs advisory, shown only when nothing blocks.
fn findings(file: &EditedFile, text: &str, ctx: &RuleContext<'_>) -> QualityFindings {
  let f = PyFile::new(file, text, limits(ctx));
  QualityFindings {
    errors: errors(&f, ctx),
    advisories: Some(docs(&f))
      .filter(|text| !text.is_empty())
      .into_iter()
      .collect(),
  }
}

impl QualityRule for PythonQuality {
  fn language(&self) -> &'static str {
    "python"
  }

  fn extensions(&self) -> &[&str] {
    &["py"]
  }

  fn source(&self) -> &'static str {
    "python-quality-hook"
  }

  fn reason(&self) -> &'static str {
    "Post-edit Python quality violation(s) detected"
  }

  fn project_enabled(&self, ctx: &RuleContext<'_>) -> bool {
    detect_python(ctx.env, cwd(ctx)) && tool_available("python3", ctx.env)
  }

  fn skip_linked_worktrees(&self) -> bool {
    false
  }

  fn ast_rule_dirs(&self) -> Vec<PathBuf> {
    Vec::new()
  }

  /// The batch scan is always empty here: no-mocks scans each test file on its
  /// own (see `no_mocks`), so editing any other Python file spawns no ast-grep.
  fn check(
    &self,
    file: &EditedFile,
    ctx: &RuleContext<'_>,
    _scan: &AstGrepScan,
  ) -> QualityFindings {
    match read(file) {
      Ok(text) => findings(file, &text, ctx),
      Err(error) => QualityFindings {
        errors: vec![error],
        advisories: Vec::new(),
      },
    }
  }
}

#[cfg(test)]
#[path = "tests/rule_test.rs"]
mod tests;
