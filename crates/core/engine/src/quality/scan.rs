//! One structural scan for all quality rules and edited files in an event.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_runtime::process::{Output, Spec, run};
use toolu_runtime::registry::rule::RuleContext;

use super::EditedFile;

/// One source line matched by a named structural rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstGrepHit {
  /// The rule's id.
  pub rule_id: String,
  /// The matched file as ast-grep printed it.
  pub file: String,
  /// One-based source line.
  pub line: i64,
  /// `<file>:<line>:<source line>`.
  pub excerpt: String,
  /// The source line alone.
  pub text: String,
  /// Whether this is the first line of its match.
  pub first: bool,
}

/// Where a scan failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStage {
  /// The executable or its rules failed.
  AstGrep,
  /// Its stdout was not a valid JSON match array.
  Parse,
}

/// A failed scan, with the shell-compatible status and capped first stderr line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFailure {
  /// Failure stage.
  pub stage: ScanStage,
  /// Process exit status, or 128 plus its signal.
  pub exit_code: i32,
  /// First stderr line, capped at 200 characters.
  pub stderr_first: String,
}

/// Result of one ast-grep scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstGrepScan {
  /// ast-grep is absent from PATH.
  Missing,
  /// Parsed output; `empty` distinguishes blank stdout from `[]`.
  Ok {
    /// Flattened, rule-tagged source lines.
    hits: Vec<AstGrepHit>,
    /// Whether stdout was blank.
    empty: bool,
  },
  /// The process or its output failed.
  Failed(ScanFailure),
}

fn failed(stage: ScanStage, exit_code: i32, stderr: &str) -> AstGrepScan {
  let stderr_first = stderr
    .lines()
    .next()
    .unwrap_or_default()
    .chars()
    .take(200)
    .collect();
  AstGrepScan::Failed(ScanFailure {
    stage,
    exit_code,
    stderr_first,
  })
}

fn binary(ctx: &RuleContext<'_>) -> Option<PathBuf> {
  let cwd = ctx.cwd.unwrap_or(ctx.project_root);
  ctx.env.get("PATH")?.split(':').find_map(|part| {
    let dir = if part.is_empty() {
      Path::new(".")
    } else {
      Path::new(part)
    };
    let dir = if dir.is_absolute() {
      dir.to_path_buf()
    } else {
      cwd.join(dir)
    };
    let path = dir.join("ast-grep");
    std::fs::metadata(&path)
      .ok()
      .filter(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
      .map(|_| path)
  })
}

fn interpolated(value: Option<&Value>) -> String {
  value.map_or_else(
    || "null".to_owned(),
    |value| {
      value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
    },
  )
}

fn match_lines(value: &Value) -> Option<Vec<AstGrepHit>> {
  let lines = match value.get("lines") {
    None | Some(Value::Null | Value::Bool(false)) => "",
    Some(Value::String(lines)) => lines,
    Some(_) => return None,
  };
  let start = value.get("range")?.get("start")?.get("line")?.as_i64()?;
  let file = interpolated(value.get("file"));
  let rule_id = interpolated(value.get("ruleId"));
  let texts = if lines.is_empty() {
    Vec::new()
  } else {
    lines.split('\n').collect()
  };
  Some(
    texts
      .into_iter()
      .enumerate()
      .map(|(index, text)| {
        let line = start + 1 + i64::try_from(index).unwrap_or(i64::MAX - start - 1);
        AstGrepHit {
          rule_id: rule_id.clone(),
          file: file.clone(),
          line,
          excerpt: format!("{file}:{line}:{text}"),
          text: text.to_owned(),
          first: index == 0,
        }
      })
      .collect(),
  )
}

fn parse(output: &Output) -> AstGrepScan {
  if output.timed_out || output.truncated || output.exit_code != 0 || !output.stderr.is_empty() {
    let reason = if output.timed_out {
      "ast-grep timed out"
    } else if output.truncated {
      "ast-grep output exceeded limit"
    } else {
      &output.stderr
    };
    return failed(ScanStage::AstGrep, output.exit_code, reason);
  }
  if output.stdout.trim().is_empty() {
    return AstGrepScan::Ok {
      hits: Vec::new(),
      empty: true,
    };
  }
  let Ok(Value::Array(matches)) = serde_json::from_str::<Value>(&output.stdout) else {
    return failed(ScanStage::Parse, output.exit_code, "");
  };
  let mut hits = Vec::new();
  for one in &matches {
    let Some(lines) = match_lines(one) else {
      return failed(ScanStage::Parse, output.exit_code, "");
    };
    hits.extend(lines);
  }
  AstGrepScan::Ok { hits, empty: false }
}

/// Scan `files` using inline YAML rules in one process.
pub fn scan_inline(files: &[EditedFile], rules: &str, ctx: &RuleContext<'_>) -> AstGrepScan {
  if files.is_empty() || rules.is_empty() {
    return AstGrepScan::Ok {
      hits: Vec::new(),
      empty: true,
    };
  }
  let Some(bin) = binary(ctx) else {
    return AstGrepScan::Missing;
  };
  let mut spec = Spec::new([
    bin.to_string_lossy().into_owned(),
    "scan".to_owned(),
    "--inline-rules".to_owned(),
    rules.to_owned(),
    "--json".to_owned(),
  ]);
  spec.argv.extend(files.iter().map(|file| file.path.clone()));
  spec.cwd = Some(ctx.cwd.unwrap_or(ctx.project_root).to_path_buf());
  spec.env = Some(ctx.env.clone());
  spec.max_output_bytes = 8 * 1024 * 1024;
  match run(&spec) {
    Ok(output) => parse(&output),
    Err(error) => failed(ScanStage::AstGrep, 1, &format!("{error:?}")),
  }
}

fn rule_text(dirs: &[&Path]) -> Result<String, String> {
  let mut all = Vec::new();
  for dir in dirs {
    let entries = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let mut files: Vec<PathBuf> = entries
      .filter_map(|entry| entry.ok().map(|entry| entry.path()))
      .collect();
    files.sort();
    for path in files {
      if !matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml")
      ) {
        continue;
      }
      if !std::fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
        continue;
      }
      all.push(std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?);
    }
  }
  Ok(all.join("\n---\n"))
}

/// Scan all YAML files in `dirs` over `files` in one process.
pub fn scan_rule_dirs(files: &[EditedFile], dirs: &[&Path], ctx: &RuleContext<'_>) -> AstGrepScan {
  match rule_text(dirs) {
    Ok(rules) => scan_inline(files, &rules, ctx),
    Err(error) => failed(ScanStage::AstGrep, 1, &error),
  }
}

#[cfg(test)]
#[path = "tests/scan_test.rs"]
mod tests;
