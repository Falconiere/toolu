//! The environment of a hermetic run: Git's isolation, the process's home and
//! temp dirs, and a PATH of the system tools plus links to the git and
//! ast-grep the process PATH resolves. Their own directories stay out: a host
//! CLI beside them (codex sits with ast-grep in Homebrew's bin) would be
//! selected to review instead of the loopback origin.

use std::path::Path;

use toolu_runtime::env::Env;

/// `dir/bin`, linking git and ast-grep, ahead of `/usr/bin:/bin`.
fn tools_path(dir: &Path) -> Result<String, String> {
  let bin = dir.join("bin");
  std::fs::create_dir_all(&bin).map_err(|err| err.to_string())?;
  let process = Env::process();
  let dirs: Vec<&str> = process.get("PATH").unwrap_or("").split(':').collect();
  for tool in ["git", "ast-grep"] {
    let link = bin.join(tool);
    // A relative PATH entry would make a link that dangles outside the cwd.
    let found = dirs
      .iter()
      .map(|dir| Path::new(dir).join(tool))
      .find(|path| path.is_absolute() && path.is_file());
    // A second environment under the same dir finds the links already made.
    if let Some(found) = found
      && std::fs::symlink_metadata(&link).is_err()
    {
      std::os::unix::fs::symlink(found, link).map_err(|err| err.to_string())?;
    }
  }
  Ok(format!("{}:/usr/bin:/bin", bin.display()))
}

/// Git's isolation, the process's home and temp dirs, and [`tools_path`].
pub(super) fn tools_env(dir: &Path) -> Result<Env, String> {
  let mut env = Env::from_pairs([
    ("GIT_CONFIG_NOSYSTEM", "1".to_owned()),
    ("GIT_CONFIG_GLOBAL", "/dev/null".to_owned()),
    ("XDG_CONFIG_HOME", "/dev/null".to_owned()),
    ("PATH", tools_path(dir)?),
  ]);
  let process = Env::process();
  for name in ["HOME", "TMPDIR"] {
    if let Some(value) = process.get(name) {
      env = env.with(name, value);
    }
  }
  Ok(env)
}
