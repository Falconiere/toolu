//! Back up, write and remove profiles. Lines match `setup.ts`.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use toolu_runtime::env::Env;

use super::{Action, PROFILES, Profile};

/// What `commit` applies: templates, destination, optional backup and the plan.
pub(super) struct Job<'a> {
  templates: Option<&'a Path>,
  agents: &'a str,
  backup: Option<&'a Path>,
  command: &'a str,
  actions: &'a [Action],
}

impl<'a> Job<'a> {
  /// The five values `commit` needs, kept off the function parameter list.
  pub(super) fn new(
    templates: Option<&'a Path>,
    agents: &'a str,
    backup: Option<&'a Path>,
    command: &'a str,
    actions: &'a [Action],
  ) -> Self {
    Self {
      templates,
      agents,
      backup,
      command,
      actions,
    }
  }
}

/// The backup directory when any profile will be replaced or removed.
pub(super) fn prepare_backup(
  env: &Env,
  agents: &str,
  actions: &[Action],
) -> Result<Option<PathBuf>, String> {
  let names = backup_names(actions);
  if names.is_empty() {
    return Ok(None);
  }
  let stamp = backup_stamp(env)?;
  let dir = PathBuf::from(format!("{agents}/.toolu-backups/{stamp}"));
  for name in names {
    let file = dir.join(format!("{name}.toml"));
    if file.exists() {
      return Err(format!("backup already exists: {}", file.display()));
    }
  }
  Ok(Some(dir))
}

/// Create directories, apply the plan and append the summary lines.
pub(super) fn commit(stdout: &mut String, job: &Job<'_>) -> Result<(), String> {
  fs::create_dir_all(job.agents).map_err(|_err| format!("could not create {}", job.agents))?;
  if let Some(dir) = job.backup {
    fs::create_dir_all(dir).map_err(|_err| format!("could not create {}", dir.display()))?;
  }
  let mut counts = Counts::default();
  for (profile, action) in PROFILES.iter().zip(job.actions) {
    counts.record(job.command, *action);
    place(profile, job, *action)?;
  }
  stdout.push('\n');
  stdout.push_str(&counts.summary(job.command));
  if let Some(dir) = job.backup {
    stdout.push('\n');
    stdout.push_str("BACKUP ");
    stdout.push_str(&dir.display().to_string());
  }
  stdout.push('\n');
  stdout.push_str("Restart Codex to reload custom agent profiles.");
  Ok(())
}

fn backup_names(actions: &[Action]) -> Vec<&'static str> {
  PROFILES
    .iter()
    .zip(actions)
    .filter_map(|(profile, action)| {
      matches!(action, Action::Update | Action::Remove | Action::Conflict).then_some(profile.name)
    })
    .collect()
}

fn backup_stamp(env: &Env) -> Result<String, String> {
  let stamp = env
    .get("TOOLU_TIMESTAMP")
    .map_or_else(utc_stamp, ToOwned::to_owned);
  if !stamp.is_empty()
    && stamp
      .chars()
      .all(|c| c.is_ascii_digit() || c == 'T' || c == 'Z')
  {
    Ok(stamp)
  } else {
    Err(format!("invalid backup timestamp: {stamp}"))
  }
}

fn utc_stamp() -> String {
  let secs = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| since.as_secs());
  let days = secs / 86_400;
  let time = secs % 86_400;
  let (year, month, day) = civil_date(days);
  format!(
    "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
    time / 3600,
    time / 60 % 60,
    time % 60
  )
}

fn civil_date(days: u64) -> (i32, u32, u32) {
  let z = days.cast_signed() + 719_468;
  let era = z.div_euclid(146_097);
  let doe = (z - era * 146_097).cast_unsigned();
  let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
  let y = yoe.cast_signed() + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let day = doy - (153 * mp + 2) / 5 + 1;
  let month = if mp < 10 { mp + 3 } else { mp - 9 };
  let year = if month <= 2 { y + 1 } else { y };
  match (
    i32::try_from(year),
    u32::try_from(month),
    u32::try_from(day),
  ) {
    (Ok(year), Ok(month), Ok(day)) => (year, month, day),
    (Err(_err), _, _) | (_, Err(_err), _) | (_, _, Err(_err)) => (1970, 1, 1),
  }
}

#[derive(Default)]
struct Counts {
  installed: u32,
  updated: u32,
  unchanged: u32,
  removed: u32,
  absent: u32,
}

impl Counts {
  fn record(&mut self, command: &str, action: Action) {
    if command == "install" {
      self.record_install(action);
    } else {
      self.record_remove(action);
    }
  }

  fn record_install(&mut self, action: Action) {
    match action {
      Action::Unchanged => self.unchanged += 1,
      Action::Update | Action::Conflict => self.updated += 1,
      Action::Install => self.installed += 1,
      Action::Absent | Action::Remove => {}
    }
  }

  fn record_remove(&mut self, action: Action) {
    if action == Action::Absent {
      self.absent += 1;
    } else if action == Action::Remove || action == Action::Conflict {
      self.removed += 1;
    }
  }

  fn summary(&self, command: &str) -> String {
    if command == "install" {
      format!(
        "INSTALLED {} UPDATED {} UNCHANGED {}",
        self.installed, self.updated, self.unchanged
      )
    } else {
      format!("REMOVED {} ABSENT {}", self.removed, self.absent)
    }
  }
}

fn place(profile: &Profile, job: &Job<'_>, action: Action) -> Result<(), String> {
  let target = format!("{}/{}.toml", job.agents, profile.name);
  if job.command == "install" {
    return install_one(job, profile, &target, action);
  }
  if action == Action::Remove || action == Action::Conflict {
    return move_to_backup(&target, job.backup, profile.name);
  }
  Ok(())
}

fn install_one(
  job: &Job<'_>,
  profile: &Profile,
  target: &str,
  action: Action,
) -> Result<(), String> {
  if action == Action::Update || action == Action::Conflict {
    copy_to_backup(target, job.backup, profile.name)?;
    return write_profile(job.templates, profile, target);
  }
  if action == Action::Install {
    return write_profile(job.templates, profile, target);
  }
  Ok(())
}

fn copy_to_backup(target: &str, backup: Option<&Path>, name: &str) -> Result<(), String> {
  let Some(dir) = backup else {
    return Err("backup directory missing".to_owned());
  };
  fs::copy(target, dir.join(format!("{name}.toml")))
    .map(|_| ())
    .map_err(|_err| format!("could not back up {target}"))
}

fn move_to_backup(target: &str, backup: Option<&Path>, name: &str) -> Result<(), String> {
  let Some(dir) = backup else {
    return Err("backup directory missing".to_owned());
  };
  fs::rename(target, dir.join(format!("{name}.toml")))
    .map_err(|_err| format!("could not back up {target}"))
}

fn write_profile(dir: Option<&Path>, profile: &Profile, target: &str) -> Result<(), String> {
  let text = super::template_text(dir, profile)?;
  let path = Path::new(target);
  let parent = path.parent().unwrap_or(Path::new("."));
  let tmp = parent.join(format!(
    ".{}.toml.toolu.{}",
    profile.name,
    std::process::id()
  ));
  let mut file = fs::File::create(&tmp).map_err(|_err| format!("could not write {target}"))?;
  file
    .write_all(text.as_bytes())
    .map_err(|_err| format!("could not write {target}"))?;
  fs::rename(&tmp, path).map_err(|_err| format!("could not write {target}"))
}

#[cfg(test)]
#[path = "tests/apply_test.rs"]
mod tests;
