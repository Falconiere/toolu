//! The compiled-in rust-quality rule: the registry [`Rule`] its manifest
//! enables, and the [`QualityRule`] the shared post-edit runner drives for
//! every edited `.rs` file of a Cargo project with cargo on PATH.

use std::path::{Path, PathBuf};

use toolu_engine::quality::{
  AstGrepScan, EditedFile, ManifestRule, QualityFindings, QualityRule, scan_inline,
};
use toolu_runtime::config::load::load;
use toolu_runtime::config::quality::{QualityLang, quality_flag, quality_threshold};
use toolu_runtime::config::settings::{RUST_UNSAFE_EXEMPTIONS, read_list, settings_dir};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::rule::{Rule, RuleContext};
use toolu_state::detect::project::detect_rust;
use toolu_state::detect::tools::tool_available;

use crate::ast_rules::{error_handling, error_rules, mock_rules, no_mocks};
use crate::docs::docs;
use crate::file::{Limits, RsFile};
use crate::layout_rules::{is_rust_test, test_layout};
use crate::line_rules::{suppression, unsafe_code};
use crate::size_rules::{file_too_long, function_too_long, impl_too_long};

/// rust-quality's post-edit rule.
pub(crate) struct RustQuality;

/// The rule its `rust-quality@toolu__rust-quality.json` manifest enables.
pub const RUST_QUALITY: &dyn Rule = &ManifestRule {
  spec: SPEC,
  name: NAME,
  rule: RustQuality,
};

/// The manifest spec.
pub(crate) const SPEC: &str = "rust-quality@toolu";
/// The manifest and rule name.
pub(crate) const NAME: &str = "rust-quality";

fn cwd<'a>(ctx: &RuleContext<'a>) -> &'a Path {
  ctx.cwd.unwrap_or(ctx.project_root)
}

/// The limits of `ctx`'s project; an unreadable exemptions list exempts nothing.
pub(crate) fn limits(ctx: &RuleContext<'_>) -> Limits {
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let config = load(&roots, ctx.cwd);
  let threshold = |key| quality_threshold(&config, QualityLang::Rust, key, None);
  let unsafe_exemptions = settings_dir(&roots, None)
    .and_then(|dir| read_list(&dir.join(RUST_UNSAFE_EXEMPTIONS)).ok())
    .unwrap_or_default();
  Limits {
    file_lines: threshold("maxFileLines"),
    fn_lines: threshold("maxFnLines"),
    impl_lines: threshold("maxImplLines"),
    no_mocks: quality_flag(&config, QualityLang::Rust, "noMocks", true),
    unsafe_exemptions,
  }
}

/// `file`'s contents, or the violation that it cannot be read.
pub(crate) fn read(file: &EditedFile) -> Result<String, String> {
  std::fs::read(&file.absolute)
    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    .map_err(|error| format!("Cannot read {} for Rust quality checks: {error}", file.path))
}

/// The structural scans of one file; `None` where its rules do not apply.
#[derive(Debug, Default)]
pub(crate) struct Scans {
  /// The scan with [`error_rules`].
  pub(crate) errors: Option<AstGrepScan>,
  /// The scan with [`mock_rules`].
  pub(crate) mocks: Option<AstGrepScan>,
}

/// Every rule over `f` in the numeric order of the bash fragments it replaces;
/// the docs advisories count only when nothing blocks.
pub(crate) fn findings(f: &RsFile<'_>, is_test: bool, scans: &Scans) -> QualityFindings {
  let errors = [
    file_too_long(f),
    test_layout(f, is_test),
    suppression(f),
    unsafe_code(f),
    function_too_long(f, is_test),
    impl_too_long(f, is_test),
  ]
  .into_iter()
  .flatten()
  .chain(error_handling(f, scans.errors.as_ref()))
  .chain(no_mocks(f, scans.mocks.as_ref()))
  .collect();
  QualityFindings {
    errors,
    advisories: docs(f, is_test),
  }
}

impl QualityRule for RustQuality {
  fn language(&self) -> &'static str {
    "rust"
  }

  fn extensions(&self) -> &[&str] {
    &["rs"]
  }

  fn source(&self) -> &'static str {
    "rust-quality-hook"
  }

  fn reason(&self) -> &'static str {
    "Post-edit Rust quality violation(s) detected"
  }

  fn project_enabled(&self, ctx: &RuleContext<'_>) -> bool {
    detect_rust(ctx.env, cwd(ctx)) && tool_available("cargo", ctx.env)
  }

  fn skip_linked_worktrees(&self) -> bool {
    false
  }

  fn ast_rule_dirs(&self) -> Vec<PathBuf> {
    Vec::new()
  }

  fn check(
    &self,
    file: &EditedFile,
    ctx: &RuleContext<'_>,
    _scan: &AstGrepScan,
  ) -> QualityFindings {
    let text = match read(file) {
      Ok(text) => text,
      Err(error) => {
        return QualityFindings {
          errors: vec![error],
          advisories: Vec::new(),
        };
      }
    };
    let limits = limits(ctx);
    let f = RsFile {
      file,
      scope: &file.path,
      lines: crate::file::split_lines(&text),
      limits: &limits,
      env: ctx.env,
      cwd: cwd(ctx),
    };
    let is_test = is_rust_test(&f);
    let one = std::slice::from_ref(file);
    let scans = Scans {
      errors: error_rules(&f, is_test).map(|rules| scan_inline(one, rules, ctx)),
      mocks: mock_rules(&f).map(|rules| scan_inline(one, &rules, ctx)),
    };
    findings(&f, is_test, &scans)
  }
}

#[cfg(test)]
#[path = "tests/rule_test.rs"]
mod tests;
