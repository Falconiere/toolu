//! The non-blocking advisories: code duplication through the project's jscpd
//! (90), missing or verbose `JSDoc` on exported API (92), and `await` with no
//! handler in the file (94). They never touch the gate.

use std::path::Path;
use std::time::Duration;

use toolu_runtime::process::{Spec, Wait, run};
use toolu_state::detect::project::NodePackageManager;
use toolu_state::detect::tools::tool_available;

use crate::file::{Pattern, TsFile, compile, head, is_test_path};

/// What the duplication advisory tells the agent to run.
fn typecheck_command(pm: NodePackageManager) -> &'static str {
  match pm {
    NodePackageManager::Bun => "bun run typecheck",
    NodePackageManager::Pnpm => "pnpm -w typecheck",
    NodePackageManager::Yarn => "yarn typecheck",
    NodePackageManager::Npm => "npm run typecheck",
  }
}

/// The package runner and its argv prefix for `jscpd`.
fn runner(pm: NodePackageManager) -> (&'static str, &'static [&'static str]) {
  match pm {
    NodePackageManager::Bun => ("bunx", &["bunx", "jscpd"]),
    NodePackageManager::Pnpm => ("pnpm", &["pnpm", "dlx", "jscpd"]),
    NodePackageManager::Yarn => ("yarn", &["yarn", "dlx", "jscpd"]),
    NodePackageManager::Npm => ("npx", &["npx", "jscpd"]),
  }
}

/// A basic regular expression (grep's default): `+?(){}|` are literals, and
/// `^`/`$` anchor only at the pattern's start/end.
pub(crate) fn bre(text: &str) -> String {
  let last = text.chars().count().saturating_sub(1);
  text
    .chars()
    .enumerate()
    .map(|(at, char)| match char {
      '+' | '?' | '(' | ')' | '{' | '}' | '|' => format!("\\{char}"),
      '^' if at > 0 => r"\^".to_owned(),
      '$' if at < last => r"\$".to_owned(),
      other => other.to_string(),
    })
    .collect()
}

/// `timeout 10 <runner> <pkg> --config .jscpd.json 2>&1`, both streams.
fn jscpd_output(f: &TsFile<'_>, argv: &[&str], pkg: &str, config: &Path) -> String {
  let mut spec = Spec::new(argv.iter().copied());
  spec.argv.push(pkg.to_owned());
  spec.argv.push("--config".to_owned());
  spec.argv.push(config.display().to_string());
  spec.cwd = Some(f.at.cwd.to_path_buf());
  spec.env = Some(f.at.env.clone());
  spec.timeout = Duration::from_secs(10);
  spec.wait = Wait::Streams;
  let Ok(output) = run(&spec) else {
    return String::new();
  };
  let separator = if output.stdout.is_empty() || output.stdout.ends_with('\n') {
    ""
  } else {
    "\n"
  };
  format!("{}{separator}{}", output.stdout, output.stderr)
}

static CLONE_REPORT: Pattern = Pattern::new("(?i)found.*clone|duplicat");
static TEST_OR_SPEC: Pattern = Pattern::new(r"\.(test|spec)\.");

/// 90-duplication: only for a clean non-test file under `apps/*` or `packages/*`.
pub(crate) fn duplication(f: &TsFile<'_>) -> String {
  let path = f.path();
  if TEST_OR_SPEC.is_match(path) {
    return String::new();
  }
  let root = f.at.project_root.display().to_string();
  let relative = path.strip_prefix(&format!("{root}/")).unwrap_or(path);
  if !relative.starts_with("apps/") && !relative.starts_with("packages/") {
    return String::new();
  }
  let pkg = format!(
    "{root}/{}",
    relative.split('/').take(2).collect::<Vec<_>>().join("/")
  );
  let (tool, argv) = runner(f.pm);
  let config = f.at.project_root.join(".jscpd.json");
  if !tool_available(tool, f.at.env)
    || !std::fs::metadata(&pkg).is_ok_and(|meta| meta.is_dir())
    || !std::fs::metadata(&config).is_ok_and(|meta| meta.is_file())
  {
    return String::new();
  }
  let report = jscpd_output(f, argv, &pkg, &config);
  let base = path.rsplit('/').next().unwrap_or(path);
  let named = compile(&bre(base));
  let cloned = report.split('\n').any(|line| CLONE_REPORT.is_match(line));
  let mentions = report
    .split('\n')
    .any(|line| named.as_ref().is_some_and(|regex| regex.is_match(line)));
  if !cloned || !mentions {
    return String::new();
  }
  format!(
    "Code duplication detected involving {path} — deduplicate or run '{}' before commit",
    typecheck_command(f.pm)
  )
}

static EXPORTS: [Pattern; 5] = [
  Pattern::new("^export (async )?function "),
  Pattern::new("^export (abstract )?class "),
  Pattern::new("^export default "),
  Pattern::new("^export (const|interface|type|enum) [A-Z]"),
  Pattern::new(r"^export const [a-z_][A-Za-z0-9_]* = (async )?(\(|function)"),
];
static BLANK: Pattern = Pattern::new("^[[:space:]]*$");
static LINE_COMMENT: Pattern = Pattern::new("^[[:space:]]*//");
static DOC_END: Pattern = Pattern::new(r"\*/[[:space:]]*$");
static DOC_START: Pattern = Pattern::new(r"^[[:space:]]*/\*\*");

/// Exported API whose previous code line neither closes nor opens a `JSDoc`.
fn undocumented(lines: &[String]) -> Vec<String> {
  let mut rows = Vec::new();
  let mut prev = "";
  for (at, line) in lines.iter().enumerate() {
    if BLANK.is_match(line) || LINE_COMMENT.is_match(line) {
      continue;
    }
    let documented = DOC_END.is_match(prev) || DOC_START.is_match(prev);
    if EXPORTS.iter().any(|pattern| pattern.is_match(line)) && !documented {
      rows.push(format!("{}: {line}", at + 1));
    }
    prev = line;
  }
  rows
}

/// `JSDoc` blocks longer than 12 lines; a `/**` inside a block does not restart it.
fn verbose(lines: &[String]) -> Vec<String> {
  let mut rows = Vec::new();
  let mut block: Option<(usize, usize)> = None;
  for (at, line) in lines.iter().enumerate() {
    if block.is_none() && line.contains("/**") {
      block = Some((at + 1, 0));
    }
    let Some((start, count)) = block.as_mut() else {
      continue;
    };
    *count += 1;
    if !line.contains("*/") {
      continue;
    }
    if *count > 12 {
      rows.push(format!(
        "{start}: JSDoc block is {count} lines — trim to the essentials"
      ));
    }
    block = None;
  }
  rows
}

/// 92-docs: skips tests, declarations and `index.ts(x)` barrels.
pub(crate) fn docs(f: &TsFile<'_>) -> String {
  let path = f.path();
  let name = path.rsplit('/').next().unwrap_or(path);
  if is_test_path(path) || path.ends_with(".d.ts") || name == "index.ts" || name == "index.tsx" {
    return String::new();
  }
  let missing = head(&undocumented(&f.lines), 3);
  let long = head(&verbose(&f.lines), 2);
  let mut parts = Vec::new();
  if !missing.is_empty() {
    parts.push(format!(
      "Exported API missing a JSDoc in {path} — add a concise /** */ doc:\n{missing}"
    ));
  }
  if !long.is_empty() {
    parts.push(format!(
      "Verbose JSDoc in {path} — docs must be present but concise:\n{long}"
    ));
  }
  parts.join("\n")
}

static COMMENT_LINE: Pattern = Pattern::new(r"^[[:space:]]*(//|/\*|\*)");
static AWAIT: Pattern = Pattern::new(r"\bawait[[:space:]]");
static HANDLED: Pattern = Pattern::new(r"\btry\b|\.catch\(");

/// 94-handler: `await` in code lines, and no `try` or `.catch(` in any.
pub(crate) fn unhandled_await(f: &TsFile<'_>) -> String {
  let code = || f.lines.iter().filter(|line| !COMMENT_LINE.is_match(line));
  let awaits = code().any(|line| AWAIT.is_match(line));
  let handled = code().any(|line| HANDLED.is_match(line));
  if !awaits || handled {
    return String::new();
  }
  format!(
    "Async code in {} uses await with no try/catch or .catch in the file — ensure rejections are handled here or by every caller.",
    f.path()
  )
}

#[cfg(test)]
#[path = "tests/advisories_test.rs"]
mod tests;
