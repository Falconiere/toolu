//! The line counters against TypeScript's `detect-lines.ts`: one `bun` process counts
//! every file, and the same files are counted here.

use std::path::{Path, PathBuf};
use std::process::Command;

use toolu_state::detect::lines::{
  count_code_lines, count_python_code_lines, has_unterminated_block,
};

use super::scratch::{Res, write};

/// The code lines, the Python code lines and whether a block is left open.
pub(crate) type Counts = (Option<u64>, Option<u64>, bool);

/// What `bun` runs: the counters of the TypeScript port over a NUL-separated file list.
const SCRIPT: &str = r#"
const lines = await import(process.env.DETECT_LINES);
const list = await Bun.file(process.env.DETECT_LIST).text();
const rows = list.split("\0").filter((file) => file !== "").map((file) => [
  lines.countCodeLines(file) ?? null,
  lines.countPythonCodeLines(file) ?? null,
  lines.hasUnterminatedBlock(file),
]);
process.stdout.write(JSON.stringify(rows));
"#;

/// The repository root.
pub(crate) fn repo_root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// `git ls-files -z '*.ts' '*.rs' '*.py' '*.sh'` of the repository, as absolute paths.
pub(crate) fn tracked_sources() -> Res<Vec<PathBuf>> {
  let repo = repo_root();
  let out = Command::new("git")
    .arg("-C")
    .arg(&repo)
    .args(["ls-files", "-z", "*.ts", "*.rs", "*.py", "*.sh"])
    .output()
    .map_err(|err| format!("git ls-files: {err}"))?;
  if !out.status.success() {
    return Err(format!(
      "git ls-files failed: {}",
      String::from_utf8_lossy(&out.stderr)
    ));
  }
  let names = String::from_utf8(out.stdout).map_err(|err| err.to_string())?;
  Ok(
    names
      .split('\0')
      .filter(|name| !name.is_empty())
      .map(|name| repo.join(name))
      .collect(),
  )
}

/// `files`, counted here.
pub(crate) fn rust_counts(files: &[PathBuf]) -> Vec<Counts> {
  let count = |file: &PathBuf| {
    (
      count_code_lines(file),
      count_python_code_lines(file),
      has_unterminated_block(file),
    )
  };
  files.iter().map(count).collect()
}

/// `files`, counted by `detect-lines.ts` in one `bun` process. A missing `bun` is an
/// error, never a skip.
pub(crate) fn typescript_counts(files: &[PathBuf], scratch: &Path) -> Res<Vec<Counts>> {
  let list: Vec<String> = files
    .iter()
    .map(|file| file.display().to_string())
    .collect();
  let list_file = scratch.join("files.list");
  write(&list_file, list.join("\0").as_bytes())?;
  let module = repo_root().join("packages/toolu-core/src/detect/detect-lines.ts");
  let module =
    std::fs::canonicalize(&module).map_err(|err| format!("{}: {err}", module.display()))?;
  let out = Command::new("bun")
    .args(["-e", SCRIPT])
    .env("DETECT_LINES", module)
    .env("DETECT_LIST", &list_file)
    .output()
    .map_err(|err| {
      format!("cannot run bun, which the TypeScript side of the parity needs: {err}")
    })?;
  if !out.status.success() {
    return Err(format!(
      "bun failed: {}",
      String::from_utf8_lossy(&out.stderr)
    ));
  }
  serde_json::from_slice(&out.stdout).map_err(|err| format!("bun printed unparsable counts: {err}"))
}
