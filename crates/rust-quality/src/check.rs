//! `toolu rust-quality check [<path>...]`: the blocking rules over every listed
//! `.rs` file of a repository, with its limits, as `cargo xtask gate` runs them.
//! Files sharing rule YAML share an `ast-grep scan`, a chunk at a time.

use std::collections::BTreeMap;
use std::path::Path;

use clap::{Arg, ArgMatches, Command};
use serde_json::{Map, json};
use toolu_engine::quality::{AstGrepScan, EditedFile, scan_inline};
use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::git::toplevel;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::process::{Spec, run as spawn};
use toolu_runtime::registry::rule::RuleContext;
use toolu_state::detect::tools::tool_available;

use crate::ast_rules::{error_rules, mock_rules};
use crate::file::{Limits, RsFile, split_lines};
use crate::layout_rules::is_rust_test;
use crate::rule::{Scans, findings, limits, read};

/// Files per `ast-grep scan`, which keeps a chunk's JSON under the output cap.
const CHUNK: usize = 64;

/// The `check` verb.
pub(crate) fn command() -> Command {
  Command::new("check")
    .about("Run the blocking Rust quality rules over a repository's .rs files")
    .long_about(
      "Run the blocking Rust quality rules over every tracked or untracked, not ignored .rs file \
       under the paths (the whole repository by default), with the limits of lang.rust in \
       toolu.config.json. Prints one block per finding. Exits 1 with findings or when git \
       fails, 69 without ast-grep.",
    )
    .arg(
      Arg::new("path")
        .value_name("PATH")
        .num_args(0..)
        .help("Directories or files to check, relative to the working directory"),
    )
}

/// One finding: the repository-relative path and the violation.
pub(crate) type Finding = (String, String);

/// What a check found.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
  /// The `.rs` files checked.
  pub(crate) files: usize,
  /// Every violation, in file then rule order.
  pub(crate) findings: Vec<Finding>,
}

/// `toolu rust-quality check`.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.display().to_string());
  }
  let cwd = match current_dir() {
    Ok(cwd) => cwd,
    Err(error) => {
      return Outcome::failed(
        Exit::Failure,
        format!("rust-quality: no working directory: {error}"),
      );
    }
  };
  let host = Roots::new(env.clone(), ctx.host).host();
  let paths: Vec<String> = matches
    .get_many::<String>("path")
    .map(|paths| paths.cloned().collect())
    .unwrap_or_default();
  match check(&env, host, &cwd, &paths) {
    Ok(report) => render(&report, ctx.json),
    Err((exit, message)) => Outcome::failed(exit, message),
  }
}

fn render(report: &Report, as_json: bool) -> Outcome {
  let exit = if report.findings.is_empty() {
    Exit::Success
  } else {
    Exit::Failure
  };
  if as_json {
    let findings: Vec<_> = report
      .findings
      .iter()
      .map(|(path, message)| json!({ "path": path, "message": message }))
      .collect();
    let doc = json!({ "files": report.files, "findings": findings });
    return Outcome {
      exit,
      stdout: Some(doc.to_string()),
      stderr: None,
    };
  }
  let summary = if report.findings.is_empty() {
    format!("rust-quality: {} files, ok", report.files)
  } else {
    format!("rust-quality: {} finding(s)", report.findings.len())
  };
  let blocks: Vec<&str> = report
    .findings
    .iter()
    .map(|(_, message)| message.as_str())
    .collect();
  Outcome {
    exit,
    stdout: (!blocks.is_empty()).then(|| blocks.join("\n")),
    stderr: Some(summary),
  }
}

/// The repository-relative `.rs` files under `paths`, sorted.
fn listed(env: &Env, cwd: &Path, paths: &[String]) -> Result<Vec<String>, String> {
  let mut spec = Spec::new([
    "git",
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
  ]);
  spec
    .argv
    .extend(["--full-name".to_owned(), "--".to_owned()]);
  if paths.is_empty() {
    spec.argv.push(":/".to_owned());
  } else {
    spec.argv.extend(paths.iter().cloned());
  }
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  spec.max_output_bytes = 64 * 1024 * 1024;
  let output = spawn(&spec).map_err(|error| format!("rust-quality: cannot run git: {error:?}"))?;
  if output.exit_code != 0 || output.truncated || output.timed_out {
    return Err(format!(
      "rust-quality: git ls-files failed in {}: {}",
      cwd.display(),
      output.stderr.trim()
    ));
  }
  let mut files: Vec<String> = output
    .stdout
    .split('\0')
    .filter(|path| {
      Path::new(path)
        .extension()
        .is_some_and(|extension| extension == "rs")
    })
    .map(str::to_owned)
    .collect();
  files.sort();
  Ok(files)
}

/// Check the `.rs` files under `paths` of the repository holding `cwd`.
///
/// # Errors
/// [`Exit::Unavailable`] without ast-grep; [`Exit::Failure`] outside a
/// repository or when git fails.
pub(crate) fn check(
  env: &Env,
  host: Host,
  cwd: &Path,
  paths: &[String],
) -> Result<Report, (Exit, String)> {
  if !tool_available("ast-grep", env) {
    let message = "rust-quality: ast-grep is not installed (npm install -g @ast-grep/cli)";
    return Err((Exit::Unavailable, message.to_owned()));
  }
  let Some(top) = toplevel(env, cwd) else {
    let message = format!(
      "rust-quality: {} is not inside a git repository",
      cwd.display()
    );
    return Err((Exit::Failure, message));
  };
  let files = listed(env, cwd, paths).map_err(|message| (Exit::Failure, message))?;
  let roots = Roots::new(env.clone(), Some(host));
  let raw = Map::new();
  let ctx = RuleContext {
    host,
    env,
    config_root: &roots.config_root(),
    project_root: &top,
    cwd: Some(&top),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  Ok(check_files(&files, &top, &ctx))
}

/// One listed file and its contents, or why they could not be read.
struct Loaded {
  file: EditedFile,
  scope: String,
  text: Result<String, String>,
}

fn load(path: &str, top: &Path) -> Loaded {
  let file = EditedFile {
    path: path.to_owned(),
    absolute: top.join(path),
    removed: false,
  };
  let text = read(&file);
  Loaded {
    file,
    scope: format!("/{path}"),
    text,
  }
}

fn rs_file<'a>(
  loaded: &'a Loaded,
  text: &'a str,
  limits: &'a Limits,
  ctx: &'a RuleContext<'a>,
) -> RsFile<'a> {
  RsFile {
    file: &loaded.file,
    scope: &loaded.scope,
    lines: split_lines(text),
    limits,
    env: ctx.env,
    cwd: ctx.project_root,
  }
}

fn check_files(paths: &[String], top: &Path, ctx: &RuleContext<'_>) -> Report {
  let limits = limits(ctx);
  let loaded: Vec<Loaded> = paths
    .iter()
    .filter(|path| top.join(path).exists())
    .map(|path| load(path, top))
    .collect();
  let mut error_jobs = Vec::new();
  let mut mock_jobs = Vec::new();
  for (at, one) in loaded.iter().enumerate() {
    let Ok(text) = &one.text else { continue };
    let f = rs_file(one, text, &limits, ctx);
    if let Some(rules) = error_rules(&f, is_rust_test(&f)) {
      error_jobs.push((at, rules.to_owned()));
    }
    if let Some(rules) = mock_rules(&f) {
      mock_jobs.push((at, rules));
    }
  }
  let mut errors = scan_groups(error_jobs, &loaded, ctx);
  let mut mocks = scan_groups(mock_jobs, &loaded, ctx);
  let mut report = Report {
    files: loaded.len(),
    findings: Vec::new(),
  };
  for (at, one) in loaded.iter().enumerate() {
    let messages = match &one.text {
      Err(error) => vec![error.clone()],
      Ok(text) => {
        let f = rs_file(one, text, &limits, ctx);
        let scans = Scans {
          errors: errors.remove(&at),
          mocks: mocks.remove(&at),
        };
        findings(&f, is_rust_test(&f), &scans).errors
      }
    };
    report.findings.extend(
      messages
        .into_iter()
        .map(|message| (one.file.path.clone(), message)),
    );
  }
  report
}

/// The scan of each job's file: files with the same YAML are scanned together,
/// [`CHUNK`] at a time, and each gets the hits on its own path.
fn scan_groups(
  jobs: Vec<(usize, String)>,
  loaded: &[Loaded],
  ctx: &RuleContext<'_>,
) -> BTreeMap<usize, AstGrepScan> {
  let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
  for (at, rules) in jobs {
    groups.entry(rules).or_default().push(at);
  }
  let mut scans = BTreeMap::new();
  for (rules, members) in groups {
    for chunk in members.chunks(CHUNK) {
      let batch: Vec<(usize, EditedFile)> = chunk
        .iter()
        .filter_map(|at| loaded.get(*at).map(|one| (*at, one.file.clone())))
        .collect();
      let files: Vec<EditedFile> = batch.iter().map(|(_, file)| file.clone()).collect();
      let scan = scan_inline(&files, &rules, ctx);
      for (at, file) in batch {
        scans.insert(at, own_hits(&scan, &file));
      }
    }
  }
  scans
}

fn own_hits(scan: &AstGrepScan, file: &EditedFile) -> AstGrepScan {
  let AstGrepScan::Ok { hits, empty } = scan else {
    return scan.clone();
  };
  let hits = hits
    .iter()
    .filter(|hit| hit.file == file.path)
    .cloned()
    .collect();
  AstGrepScan::Ok {
    hits,
    empty: *empty,
  }
}

#[cfg(test)]
#[path = "tests/check_test.rs"]
mod tests;
