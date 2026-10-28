//! `run --clone`: copy the project, as it stands, into a clone on the sandbox's
//! own disk and run the command there. The clone gets `HEAD`, the remote-tracking
//! refs (so a gate finds `origin/main`), every staged and unstaged change, and the
//! untracked files git does not ignore. Ignored build output (`target/`,
//! `node_modules/`) stays in the clone between runs, and results never flow
//! back to the checkout. One run at a time holds the clone's lock.

use std::path::Path;

/// The clone script, run as `bash -euo pipefail -c CLONE toolu-sandbox-clone
/// <src> <dst> <rel> <command>...`; the values are arguments, never spliced in.
pub(crate) const CLONE: &str = r#"src=$1 dst=$2 rel=$3
shift 3
mkdir -p "$dst"
if command -v flock >/dev/null 2>&1; then
  exec 9>"$dst.lock"
  flock -n 9 || { echo 'toolu-sandbox: waiting for another run in this clone' >&2; flock 9; }
fi
cd "$dst"
[ -d .git ] || git init -q
git fetch -q --no-tags "$src" '+refs/remotes/*:refs/remotes/*'
git fetch -q --no-tags "$src" HEAD
git checkout -q --force --detach FETCH_HEAD
git clean -fdq
git -C "$src" --no-optional-locks diff --binary --no-color --no-ext-diff --no-textconv --src-prefix=a/ --dst-prefix=b/ HEAD | git apply --allow-empty --whitespace=nowarn
list=$(mktemp)
git -C "$src" ls-files -z --others --exclude-standard >"$list"
if [ -s "$list" ]; then
  tar -C "$src" --null -T "$list" -cf - | tar -C "$dst" -xf -
fi
rm -f "$list"
cd "$dst/$rel"
exec "$@"
"#;

/// The argv that runs `command` in a clone of `src` at `dst`, from `rel` inside it.
pub(crate) fn clone_argv(src: &Path, dst: &Path, rel: &Path, command: &[String]) -> Vec<String> {
  let mut argv: Vec<String> = [
    "bash",
    "-euo",
    "pipefail",
    "-c",
    CLONE,
    "toolu-sandbox-clone",
  ]
  .into_iter()
  .map(str::to_owned)
  .collect();
  for path in [src, dst, rel] {
    argv.push(path.to_string_lossy().into_owned());
  }
  argv.extend(command.iter().cloned());
  argv
}

#[cfg(test)]
#[path = "tests/clone_test.rs"]
mod tests;
