//! The process environment a command reads: its argv, its own path and its
//! working directory. Only this
//! crate reads `std::env` (rule 14).

use std::path::PathBuf;

/// The argv after the program name; a non-UTF-8 word is replaced lossily.
pub fn args() -> Vec<String> {
  std::env::args_os()
    .skip(1)
    .map(|word| word.to_string_lossy().into_owned())
    .collect()
}

/// This executable's canonical path, or `None` when the OS cannot say.
pub fn current_exe() -> Option<PathBuf> {
  std::env::current_exe().and_then(std::fs::canonicalize).ok()
}

/// The process working directory, or `.` when the OS cannot say.
pub fn current_dir() -> PathBuf {
  std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
#[path = "tests/invocation_test.rs"]
mod tests;
