//! `toolu ast-grep outline <path>...`: a file prints its items with ranges, a
//! directory prints one line per file with its public names.

use std::path::Path;

use clap::ArgMatches;
use serde_json::{Value, json};
use toolu_engine::outline::{FileOutline, MISSING, outline, supported};
use toolu_engine::quality::binary;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::process::{Spec, run as spawn};

const MAX_ITEMS: usize = 150;
const MAX_FILES: usize = 150;
const MAX_NAMES: usize = 12;
const MAX_TEXT: usize = 12 * 1024;
const TRUNCATED: &str = "... output truncated at 12 KB; outline fewer paths";

/// One command-line path: a file, or a directory's listed files.
enum Target {
  File(String),
  Dir {
    dir: String,
    files: Vec<String>,
    omitted: usize,
  },
}

fn usage(message: &str) -> Outcome {
  Outcome::failed(Exit::Usage, format!("ast-grep outline: {message}"))
}

/// The supported files git lists under `dir`, sorted.
fn listed(env: &Env, cwd: &Path, dir: &str) -> Result<Vec<String>, String> {
  let mut spec = Spec::new([
    "git",
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
    "--",
    dir,
  ]);
  spec.cwd = Some(cwd.to_path_buf());
  spec.env = Some(env.clone());
  spec.max_output_bytes = 64 * 1024 * 1024;
  let output = spawn(&spec).map_err(|error| format!("git ls-files failed: {error:?}"))?;
  if output.exit_code != 0 || output.truncated || output.timed_out {
    let first = output.stderr.lines().next().unwrap_or_default().to_owned();
    return Err(format!("git ls-files failed: {first}"));
  }
  let mut files: Vec<String> = output
    .stdout
    .split('\0')
    .filter(|path| !path.is_empty() && supported(path) && cwd.join(path).is_file())
    .map(str::to_owned)
    .collect();
  // git lists untracked files after tracked ones.
  files.sort();
  Ok(files)
}

fn targets(env: &Env, cwd: &Path, paths: &[String]) -> Result<Vec<Target>, Outcome> {
  let mut targets = Vec::with_capacity(paths.len());
  for path in paths {
    let full = cwd.join(path);
    if full.is_dir() {
      let mut files = listed(env, cwd, path)
        .map_err(|error| Outcome::failed(Exit::Failure, format!("ast-grep outline: {error}")))?;
      let omitted = files.len().saturating_sub(MAX_FILES);
      files.truncate(MAX_FILES);
      targets.push(Target::Dir {
        dir: path.clone(),
        files,
        omitted,
      });
    } else if !full.is_file() {
      return Err(usage(&format!("{path}: no such file or directory")));
    } else if !supported(path) {
      return Err(usage(&format!("unsupported language: {path}")));
    } else {
      targets.push(Target::File(path.clone()));
    }
  }
  Ok(targets)
}

fn file_text(file: &FileOutline) -> String {
  let mut out = vec![format!(
    "{}: {} lines, {} items",
    file.path,
    file.lines,
    file.items.len()
  )];
  out.extend(file.items.iter().take(MAX_ITEMS).map(|item| {
    let indent = "  ".repeat(item.depth);
    let doc = item
      .doc
      .as_ref()
      .map_or_else(String::new, |doc| format!(" // {doc}"));
    format!(
      "{indent}{}-{} {}{doc}",
      item.start, item.end, item.signature
    )
  }));
  let more = file.items.len().saturating_sub(MAX_ITEMS);
  if more > 0 {
    out.push(format!("... {more} more items"));
  }
  out.join("\n")
}

/// Depth-0 public names, at most [`MAX_NAMES`], and how many more there are.
fn names(file: &FileOutline) -> (Vec<&str>, usize) {
  let all: Vec<&str> = file
    .items
    .iter()
    .filter(|item| item.depth == 0 && item.public)
    .map(|item| item.name.as_str())
    .collect();
  let more = all.len().saturating_sub(MAX_NAMES);
  (all.into_iter().take(MAX_NAMES).collect(), more)
}

fn dir_text(dir: &str, files: &[&FileOutline], omitted: usize) -> String {
  let mut out = vec![format!("{dir}: {} files", files.len() + omitted)];
  out.extend(files.iter().map(|file| {
    let (shown, more) = names(file);
    let names = if shown.is_empty() {
      String::new()
    } else {
      format!(": {}", shown.join(", "))
    };
    let more = if more > 0 {
      format!(", +{more} more")
    } else {
      String::new()
    };
    format!(
      "{}: {} lines, {} items{names}{more}",
      file.path,
      file.lines,
      file.items.len()
    )
  }));
  if omitted > 0 {
    out.push(format!("... {omitted} more files"));
  }
  out.join("\n")
}

/// `text` cut at its last whole line within [`MAX_TEXT`] bytes, with a notice.
fn capped(text: String) -> String {
  if text.len() <= MAX_TEXT {
    return text;
  }
  let room = MAX_TEXT - TRUNCATED.len() - 1;
  let cut = text
    .get(..=room)
    .and_then(|head| head.rfind('\n'))
    .unwrap_or(0);
  format!("{}\n{TRUNCATED}", text.get(..cut).unwrap_or_default())
}

fn file_json(file: &FileOutline) -> Value {
  let items: Vec<Value> = file
    .items
    .iter()
    .take(MAX_ITEMS)
    .map(|item| {
      json!({
        "name": item.name, "signature": item.signature, "doc": item.doc,
        "start": item.start, "end": item.end, "depth": item.depth, "public": item.public
      })
    })
    .collect();
  let omitted = file.items.len().saturating_sub(MAX_ITEMS);
  json!({"path": file.path, "lines": file.lines, "items": items, "omitted": omitted})
}

fn dir_json(dir: &str, files: &[&FileOutline], omitted: usize) -> Value {
  let files: Vec<Value> = files
    .iter()
    .map(|file| {
      let (shown, _) = names(file);
      json!({"path": file.path, "lines": file.lines, "items": file.items.len(), "names": shown})
    })
    .collect();
  json!({"dir": dir, "files": files, "omitted": omitted})
}

fn render(targets: &[Target], outlines: &[FileOutline], json_output: bool) -> String {
  let find = |path: &str| outlines.iter().find(|file| file.path == path);
  let mut texts = Vec::with_capacity(targets.len());
  let mut docs = Vec::with_capacity(targets.len());
  for target in targets {
    match target {
      Target::File(path) => {
        if let Some(file) = find(path) {
          texts.push(file_text(file));
          docs.push(file_json(file));
        }
      }
      Target::Dir {
        dir,
        files,
        omitted,
      } => {
        let found: Vec<&FileOutline> = files.iter().filter_map(|path| find(path)).collect();
        texts.push(dir_text(dir, &found, *omitted));
        docs.push(dir_json(dir, &found, *omitted));
      }
    }
  }
  if json_output {
    json!({"paths": docs}).to_string()
  } else {
    capped(texts.join("\n\n"))
  }
}

/// Run `toolu ast-grep outline`.
pub(crate) fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let paths: Vec<String> = matches
    .get_many::<String>("paths")
    .into_iter()
    .flatten()
    .cloned()
    .collect();
  let env = Env::process();
  let cwd = match current_dir() {
    Ok(cwd) => cwd,
    Err(error) => return Outcome::failed(Exit::Failure, format!("ast-grep outline: {error}")),
  };
  if binary(&env, &cwd).is_none() {
    return Outcome::failed(Exit::Unavailable, format!("ast-grep outline: {MISSING}"));
  }
  let targets = match targets(&env, &cwd, &paths) {
    Ok(targets) => targets,
    Err(outcome) => return outcome,
  };
  let files: Vec<String> = targets
    .iter()
    .flat_map(|target| match target {
      Target::File(path) => std::slice::from_ref(path),
      Target::Dir { files, .. } => files.as_slice(),
    })
    .cloned()
    .collect();
  match outline(&env, &cwd, &files) {
    Ok(outlines) => Outcome::data(render(&targets, &outlines, ctx.json)),
    Err(error) => Outcome::failed(Exit::Failure, format!("ast-grep outline: {error}")),
  }
}

#[cfg(test)]
#[path = "tests/map_test.rs"]
mod tests;
