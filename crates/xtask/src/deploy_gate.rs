//! `cargo xtask deploy-gate`: the `verify` job of `release-native.yml` (release
//! helper unittests, then the full `cargo xtask gate`) for the `/deploy` skill,
//! in a clean Linux container fed a bundle of `HEAD`. A pass writes the receipt
//! `deploy-guard` requires before any tag, tap or release is published.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::ci_diff::git;
use crate::deploy_guard::{RECEIPT, SNAPSHOT};
use crate::options::Options;
use crate::{Verdict, output};

/// The image the gate runs in, rebuilt from [`DOCKERFILE`] on every run (layer-cached).
const IMAGE: &str = "toolu-deploy-gate";

/// The named volumes that keep cargo's registry, git checkouts and the target
/// directory between runs. `docker volume rm` them to start cold.
pub(crate) const CACHES: [(&str, &str); 3] = [
  ("toolu-deploy-gate-target", "/src/target"),
  ("toolu-deploy-gate-registry", "/usr/local/cargo/registry"),
  ("toolu-deploy-gate-git", "/usr/local/cargo/git"),
];

/// The `verify` job's tools at `release-native.yml`'s pins (a test keeps them equal), plus
/// the runner-image tools the tests run for real: `npx` (the golden `bunx` shim), `jq`, `gh`.
/// Non-root uid 501 and `/src` avoid the permission and `/work` traps of a root gate.
pub(crate) const DOCKERFILE: &str = r#"ARG TOOLCHAIN
FROM node:22-trixie-slim AS node
FROM rust:${TOOLCHAIN}-trixie
ARG TOOLCHAIN
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN apt-get update -qq && apt-get install -y --no-install-recommends jq gh && rm -rf /var/lib/apt/lists/* \
 && ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
 && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
 && rustup toolchain install "$TOOLCHAIN" --profile minimal -c rustfmt,clippy,llvm-tools \
 && cargo install --locked cargo-deny@0.20.2 cargo-machete@0.9.2 cargo-llvm-cov@0.9.1 cargo-nextest@0.9.146 \
 && npm install -g @ast-grep/cli@0.50.0 \
 && useradd -m -u 501 dev \
 && git config --system user.name dev && git config --system user.email dev@toolu.invalid \
 && mkdir -p /src/target /usr/local/cargo/registry /usr/local/cargo/git \
 && chown -R dev "$CARGO_HOME" "$RUSTUP_HOME" /src
USER dev
"#;

/// Check out the bundle (the first `$BUNDLE_BYTES` bytes of stdin) in `$SRC` with
/// `origin/main`, and refuse any `HEAD` other than `$EXPECTED`, naming both. `$SRC`
/// may already hold the cached `target/` volume, so it is `git init`, not a clone.
pub(crate) const CLONE: &str = r#"B=$(mktemp)
head -c "$BUNDLE_BYTES" > "$B"
git init -q "$SRC"
cd "$SRC"
git fetch -q "$B" refs/remotes/origin/main:refs/remotes/origin/main
git fetch -q "$B" HEAD
git checkout -q --detach FETCH_HEAD
head=$(git rev-parse HEAD)
test "$head" = "$EXPECTED" || { echo "deploy-gate: the bundled HEAD is $head, not $EXPECTED" >&2; exit 1; }
"#;

/// The `verify` job itself, run in `$SRC` after [`CLONE`]. The cached target keeps only
/// dependency builds, as CI's rust-cache does: workspace artifacts are cleaned first.
pub(crate) const SCRIPT: &str = r"cargo clean --workspace --quiet
cargo clean --workspace --release --quiet
cargo llvm-cov clean --workspace
npm ci --ignore-scripts
python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'
cargo xtask gate
";

/// Run `body` in the background and stop the shell when stdin closes. The host
/// holds stdin open until the container exits, so its death, even by `SIGKILL`,
/// stops the container instead of orphaning it.
pub(crate) fn watched(body: &str) -> String {
  format!(
    "exec 3<&0\n( {body} ) &\ngate=$!\n{{ cat >/dev/null <&3; kill -TERM $$; }} &\nwait \"$gate\"\n"
  )
}

/// Run the container gate for the clean `HEAD` of `options.root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  let receipt = root.join(RECEIPT);
  snapshot(root)?;
  match std::fs::remove_file(&receipt) {
    Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
      return Err(format!("deploy-gate: cannot remove {RECEIPT}: {err}"));
    }
    _ => {}
  }
  preconditions(root)?;
  // Bundle before the long image build, so a HEAD that moves meanwhile is not gated.
  let (bundle, head) = bundle(root)?;
  let image = build(root)?;
  let opened = |err: std::io::Error| format!("deploy-gate: {}: {err}", bundle.display());
  let bytes = std::fs::metadata(&bundle).map_err(opened)?.len();
  let mut input = std::fs::File::open(&bundle).map_err(opened)?;
  let status = with_input(&mut container(&image, &head, bytes), &mut input, true)?;
  if !status.success() {
    output::error(&format!("deploy-gate: container gate failed ({status})"));
    return Ok(Verdict::Findings);
  }
  std::fs::write(&receipt, format!("{{\"commit\":\"{head}\"}}\n"))
    .map_err(|err| format!("deploy-gate: cannot write {RECEIPT}: {err}"))?;
  output::say(&format!("deploy-gate: ok {head}, receipt {RECEIPT}"));
  Ok(Verdict::Clean)
}

/// Copy this binary to [`SNAPSHOT`], which the skill's hook runs from then on: a
/// later checkout or a build failure cannot stop the guard. Renamed into place, so
/// the hook never runs a half-written file.
fn snapshot(root: &Path) -> Result<(), String> {
  let target = root.join(SNAPSHOT);
  let partial = target.with_extension("partial");
  let failed =
    |err: std::io::Error| format!("deploy-gate: cannot snapshot the guard to {SNAPSHOT}: {err}");
  if let Some(dir) = target.parent() {
    std::fs::create_dir_all(dir).map_err(failed)?;
  }
  let exe = std::env::current_exe().map_err(failed)?;
  std::fs::copy(exe, &partial).map_err(failed)?;
  std::fs::rename(&partial, &target).map_err(failed)
}

/// The worktree must be clean and `origin/main` known.
fn preconditions(root: &Path) -> Result<(), String> {
  if !git(root, &["status", "--porcelain"])?.trim().is_empty() {
    return Err("deploy-gate: the worktree is dirty; commit first".to_owned());
  }
  git(
    root,
    &["rev-parse", "-q", "--verify", "refs/remotes/origin/main"],
  )
  .map(drop)
  .map_err(|err| {
    format!("deploy-gate: refs/remotes/origin/main is missing; run git fetch origin main ({err})")
  })
}

/// The `channel` of `rust-toolchain.toml`.
pub(crate) fn channel(root: &Path) -> Result<String, String> {
  let path = root.join("rust-toolchain.toml");
  let text = std::fs::read_to_string(&path)
    .map_err(|err| format!("deploy-gate: {}: {err}", path.display()))?;
  let value: toml::Value =
    toml::from_str(&text).map_err(|err| format!("deploy-gate: {}: {err}", path.display()))?;
  value
    .get("toolchain")
    .and_then(|toolchain| toolchain.get("channel"))
    .and_then(toml::Value::as_str)
    .map(str::to_owned)
    .ok_or_else(|| format!("deploy-gate: {} has no toolchain.channel", path.display()))
}

/// Bundle `HEAD` and `origin/main` into `target/deploy/head.bundle`; the path and
/// the bundled `HEAD`.
pub(crate) fn bundle(root: &Path) -> Result<(PathBuf, String), String> {
  let dir = root.join("target/deploy");
  std::fs::create_dir_all(&dir).map_err(|err| format!("deploy-gate: {}: {err}", dir.display()))?;
  let path = dir.join("head.bundle");
  let file = path
    .to_str()
    .ok_or("deploy-gate: the bundle path is not UTF-8")?;
  git(
    root,
    &[
      "bundle",
      "create",
      "-q",
      file,
      "HEAD",
      "refs/remotes/origin/main",
    ],
  )?;
  let heads = git(root, &["bundle", "list-heads", file, "HEAD"])?;
  let head = heads
    .split_whitespace()
    .next()
    .ok_or("deploy-gate: the bundle has no HEAD")?
    .to_owned();
  Ok((path, head))
}

/// Build the image for the root's channel and return its ID, which the run uses
/// rather than the shared tag another worktree may move.
fn build(root: &Path) -> Result<String, String> {
  let id_file = root.join("target/deploy/image.id");
  let status = with_input(
    &mut build_command(&channel(root)?, &id_file),
    &mut DOCKERFILE.as_bytes(),
    false,
  )?;
  if !status.success() {
    return Err(format!("deploy-gate: docker build failed ({status})"));
  }
  std::fs::read_to_string(&id_file)
    .map(|id| id.trim().to_owned())
    .map_err(|err| format!("deploy-gate: {}: {err}", id_file.display()))
}

/// `docker build` of [`DOCKERFILE`] on stdin for `channel`, writing the image ID to `id_file`.
pub(crate) fn build_command(channel: &str, id_file: &Path) -> Command {
  let mut command = Command::new("docker");
  command
    .args(["build", "-t", &format!("{IMAGE}:{channel}"), "--build-arg"])
    .arg(format!("TOOLCHAIN={channel}"))
    .arg("--iidfile")
    .arg(id_file)
    .arg("-");
  command
}

/// `docker run` of the gate in `image` for `head` from a `bytes`-long bundle on stdin,
/// with every CPU of the Docker VM, and [`CACHES`] mounted so reruns skip most of
/// the build. The checkout itself is always fresh.
pub(crate) fn container(image: &str, head: &str, bytes: u64) -> Command {
  let mut command = Command::new("docker");
  command
    .args(["run", "--rm", "-i", "--init", "-e", "SRC=/src", "-e"])
    .arg(format!("EXPECTED={head}"))
    .arg("-e")
    .arg(format!("BUNDLE_BYTES={bytes}"));
  for (volume, path) in CACHES {
    command.arg("-v").arg(format!("{volume}:{path}"));
  }
  command
    .args([image, "sh", "-euc"])
    .arg(format!("{CLONE}{}", watched(SCRIPT)));
  command
}

/// Run `command` with `input` piped to its stdin; output is inherited. With
/// `hold_open` the pipe stays open until the child exits, so the child sees EOF
/// only if this process dies. A child that exits before reading all of its input
/// reports its own status.
pub(crate) fn with_input(
  command: &mut Command,
  input: &mut dyn Read,
  hold_open: bool,
) -> Result<ExitStatus, String> {
  let mut child = command.stdin(Stdio::piped()).spawn().map_err(|err| {
    let program = command.get_program().to_string_lossy();
    format!("deploy-gate: cannot run {program}: {err}")
  })?;
  let mut pipe = child.stdin.take();
  let written = pipe
    .as_mut()
    .map_or(Ok(0), |pipe| std::io::copy(input, pipe));
  if !hold_open {
    drop(pipe.take());
  }
  let status = child.wait().map_err(|err| format!("deploy-gate: {err}"))?;
  drop(pipe);
  match written {
    Err(err) if status.success() => Err(format!("deploy-gate: cannot write the input: {err}")),
    _ => Ok(status),
  }
}

#[cfg(test)]
#[path = "tests/deploy_gate_test.rs"]
mod tests;
