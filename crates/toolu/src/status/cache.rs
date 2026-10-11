//! Per-worktree Git counts for the latency-sensitive statusline.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use toolu_runtime::env::Env;
use toolu_runtime::git::Repo as GitRepo;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, Wait, run, spawn_orphan};

use super::repo::{Counts, counts};

const TTL_MS: u64 = 2_000;
const LOCK_AGE: Duration = Duration::from_secs(30);
const LOCK_TOKEN: &str = "TOOLU_STATUSLINE_LOCK_TOKEN";

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
  head_ref: String,
  head_oid: String,
  index_mtime_ns: u64,
  index_size: u64,
}

struct Paths {
  cache: PathBuf,
  lock: PathBuf,
}

struct Record {
  git_dir: String,
  stamp: Stamp,
  written_at_ms: u64,
  counts: Counts,
}

/// Read the last counts, scheduling one detached update when needed.
pub(super) fn read(roots: &Roots, repo: &GitRepo) -> Counts {
  let Some(paths) = paths(roots, repo) else {
    return Counts::default();
  };
  let previous = read_record(&paths.cache);
  let counts = previous
    .as_ref()
    .filter(|row| row.git_dir == repo.git_dir.to_string_lossy())
    .map_or(Counts::default(), |row| row.counts);
  let current = stamp(repo);
  let valid = previous.as_ref().is_some_and(|row| {
    row.git_dir == repo.git_dir.to_string_lossy()
      && current.as_ref() == Some(&row.stamp)
      && fresh(row.written_at_ms)
  });
  if !valid {
    schedule(roots.env(), repo, &paths);
  }
  counts
}

/// Run one real Git update in the detached process or a direct CLI invocation.
pub(super) fn refresh(roots: &Roots, repo: &GitRepo) -> Result<(), String> {
  let paths = paths(roots, repo).ok_or("statusline: no worktree state path")?;
  fs::create_dir_all(
    paths
      .cache
      .parent()
      .ok_or("statusline: no cache directory")?,
  )
  .map_err(|error| error.to_string())?;
  let token = match roots.env().get(LOCK_TOKEN) {
    Some(token) if lock_matches(&paths.lock, token) => token.to_owned(),
    Some(_) => return Err("statusline: refresh lock changed".to_owned()),
    None => match acquire(&paths.lock) {
      Some(token) => token,
      None => return Ok(()),
    },
  };
  let result = update(roots.env(), repo, &paths);
  release(&paths.lock, &token);
  result
}

fn paths(roots: &Roots, repo: &GitRepo) -> Option<Paths> {
  repo.toplevel.as_deref()?;
  let state = roots.config_root().join("toolu/statusline-cache");
  let key = repo
    .git_dir
    .as_os_str()
    .as_bytes()
    .iter()
    .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
      (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
  let cache = state.join(format!("{key:016x}.json"));
  let lock = cache.with_extension("lock");
  Some(Paths { cache, lock })
}

fn stamp(repo: &GitRepo) -> Option<Stamp> {
  let head = fs::read_to_string(repo.git_dir.join("HEAD")).ok()?;
  let head = head.trim();
  let (head_ref, head_oid) = match head.strip_prefix("ref:") {
    Some(name) => {
      let name = name.trim();
      if !name.starts_with("refs/") || name.split('/').any(|part| part == "..") {
        return None;
      }
      (name.to_owned(), ref_oid(repo, name).unwrap_or_default())
    }
    None => (String::new(), head.to_owned()),
  };
  let (index_mtime_ns, index_size) = fs::metadata(repo.git_dir.join("index"))
    .ok()
    .and_then(|meta| {
      let nanos = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
      Some((u64::try_from(nanos).ok()?, meta.len()))
    })
    .unwrap_or((0, 0));
  Some(Stamp {
    head_ref,
    head_oid,
    index_mtime_ns,
    index_size,
  })
}

fn ref_oid(repo: &GitRepo, name: &str) -> Option<String> {
  for base in [&repo.git_dir, &repo.common_dir] {
    if let Ok(text) = fs::read_to_string(base.join(name)) {
      return Some(text.trim().to_owned());
    }
  }
  let packed = fs::read_to_string(repo.common_dir.join("packed-refs")).ok()?;
  packed
    .lines()
    .filter_map(|line| line.split_once(' '))
    .find_map(|(oid, found)| (found == name).then(|| oid.to_owned()))
}

fn now_ms() -> u64 {
  millis_since_epoch(SystemTime::now())
}

fn millis_since_epoch(time: SystemTime) -> u64 {
  time
    .duration_since(UNIX_EPOCH)
    .ok()
    .and_then(|time| u64::try_from(time.as_millis()).ok())
    .unwrap_or(0)
}

fn fresh(written_at_ms: u64) -> bool {
  fresh_at(written_at_ms, now_ms())
}

fn fresh_at(written_at_ms: u64, now_ms: u64) -> bool {
  written_at_ms != 0 && now_ms != 0 && now_ms >= written_at_ms && now_ms - written_at_ms <= TTL_MS
}

fn read_record(path: &Path) -> Option<Record> {
  let value: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
  let get = |name: &str| value.get(name).and_then(Value::as_u64);
  Some(Record {
    git_dir: value.get("gitDir")?.as_str()?.to_owned(),
    stamp: Stamp {
      head_ref: value.get("headRef")?.as_str()?.to_owned(),
      head_oid: value.get("headOid")?.as_str()?.to_owned(),
      index_mtime_ns: get("indexMtimeNs")?,
      index_size: get("indexSize")?,
    },
    written_at_ms: get("writtenAtMs")?,
    counts: Counts {
      ahead: get("ahead")?,
      behind: get("behind")?,
      staged: get("staged")?,
      unstaged: get("unstaged")?,
      untracked: get("untracked")?,
    },
  })
}

fn schedule(env: &Env, repo: &GitRepo, paths: &Paths) {
  let Some(token) = acquire(&paths.lock) else {
    return;
  };
  let Some(exe) = toolu_runtime::invocation::current_exe() else {
    release(&paths.lock, &token);
    return;
  };
  let Some(top) = &repo.toplevel else {
    release(&paths.lock, &token);
    return;
  };
  let mut spec = Spec::new([
    exe.to_string_lossy().into_owned(),
    "statusline".to_owned(),
    "refresh-cache".to_owned(),
    top.to_string_lossy().into_owned(),
  ]);
  spec.cwd = Some(top.clone());
  spec.env = Some(env.clone().with(LOCK_TOKEN, &token));
  if spawn_orphan(&spec).is_err() {
    release(&paths.lock, &token);
  }
}

fn acquire(path: &Path) -> Option<String> {
  fs::create_dir_all(path.parent()?).ok()?;
  let token = format!("{}-{}", std::process::id(), now_ms());
  for _attempt in 0..2 {
    match OpenOptions::new()
      .write(true)
      .create_new(true)
      .mode(0o600)
      .open(path)
    {
      Ok(mut file) => {
        if file.write_all(token.as_bytes()).is_ok() {
          return Some(token);
        }
        let _removed = fs::remove_file(path);
        return None;
      }
      Err(_) if stale_lock(path) => {
        fs::remove_file(path).ok()?;
      }
      Err(_) => return None,
    }
  }
  None
}

fn stale_lock(path: &Path) -> bool {
  fs::metadata(path)
    .ok()
    .and_then(|meta| meta.modified().ok())
    .and_then(|modified| modified.elapsed().ok())
    .is_some_and(|age| age > LOCK_AGE)
}

fn lock_matches(path: &Path, token: &str) -> bool {
  fs::read_to_string(path).is_ok_and(|stored| stored == token)
}

fn release(path: &Path, token: &str) {
  if lock_matches(path, token) {
    let _removed = fs::remove_file(path);
  }
}

fn update(env: &Env, repo: &GitRepo, paths: &Paths) -> Result<(), String> {
  let before = stamp(repo).ok_or("statusline: cannot read HEAD")?;
  let top = repo.toplevel.as_deref().ok_or("statusline: no worktree")?;
  let mut spec = Spec::new(["git", "status", "--porcelain=v1", "--branch"]);
  spec.cwd = Some(top.to_path_buf());
  spec.env = Some(env.clone());
  spec.timeout = Duration::from_secs(5);
  spec.wait = Wait::Streams;
  let output = run(&spec).map_err(|error| format!("statusline: git: {error:?}"))?;
  if output.exit_code != 0 || output.timed_out || output.truncated {
    return Err("statusline: git status failed".to_owned());
  }
  let after = stamp(repo).ok_or("statusline: cannot reread HEAD")?;
  if (after.head_ref.as_str(), after.head_oid.as_str())
    != (before.head_ref.as_str(), before.head_oid.as_str())
  {
    return Err("statusline: HEAD changed during refresh".to_owned());
  }
  let row = Record {
    git_dir: repo.git_dir.to_string_lossy().into_owned(),
    stamp: after,
    written_at_ms: now_ms(),
    counts: counts(&output.stdout),
  };
  write_record(&paths.cache, &row).map_err(|error| error.to_string())
}

fn write_record(path: &Path, row: &Record) -> std::io::Result<()> {
  let tmp = path.with_extension(format!("{}-{}.tmp", std::process::id(), now_ms()));
  let body = json!({
    "gitDir": row.git_dir, "headRef": row.stamp.head_ref,
    "headOid": row.stamp.head_oid, "indexMtimeNs": row.stamp.index_mtime_ns,
    "indexSize": row.stamp.index_size, "writtenAtMs": row.written_at_ms,
    "ahead": row.counts.ahead, "behind": row.counts.behind,
    "staged": row.counts.staged, "unstaged": row.counts.unstaged,
    "untracked": row.counts.untracked,
  });
  let result = (|| {
    let mut file = OpenOptions::new()
      .write(true)
      .create_new(true)
      .mode(0o600)
      .open(&tmp)?;
    file.write_all(body.to_string().as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path)
  })();
  if result.is_err() {
    let _removed = fs::remove_file(&tmp);
  }
  result
}

#[cfg(test)]
#[path = "tests/cache_test.rs"]
mod tests;
