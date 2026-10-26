//! `cargo xtask deploy-gate`: the `verify` job of `release-native.yml` (release
//! helper unittests, then the full `cargo xtask gate`) for the `/deploy` skill,
//! in a clean Linux container fed a bundle of `HEAD`. A pass writes the receipt
//! `deploy-guard` requires before any tag, tap or release is published.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::ci_diff::git;
use crate::deploy_guard::RECEIPT;
use crate::options::Options;
use crate::{Verdict, output};

/// The image the gate runs in, rebuilt from [`DOCKERFILE`] on every run (layer-cached).
const IMAGE: &str = "toolu-deploy-gate";

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
 && mkdir /src && chown -R dev "$CARGO_HOME" "$RUSTUP_HOME" /src
USER dev
"#;

/// Clone the bundle on stdin into `$SRC` with `origin/main`, and refuse any
/// `HEAD` other than `$EXPECTED`, naming both.
pub(crate) const CLONE: &str = r#"B=$(mktemp)
cat > "$B"
git clone -q "$B" "$SRC"
cd "$SRC"
git fetch -q "$B" refs/remotes/origin/main:refs/remotes/origin/main
head=$(git rev-parse HEAD)
test "$head" = "$EXPECTED" || { echo "deploy-gate: the bundled HEAD is $head, not $EXPECTED" >&2; exit 1; }
"#;

/// The `verify` job itself, run in `$SRC` after [`CLONE`].
pub(crate) const SCRIPT: &str = r"npm ci --ignore-scripts
python3 -B -m unittest discover -s .github/scripts -p 'test_*.py'
cargo xtask gate
";

/// Run the container gate for the clean `HEAD` of `options.root`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  let receipt = root.join(RECEIPT);
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
  let bundle = std::fs::File::open(bundle).map_err(|err| format!("deploy-gate: {err}"))?;
  let status = with_input(&mut container(&image, &head), Stdio::from(bundle), None)?;
  if !status.success() {
    output::error(&format!("deploy-gate: container gate failed ({status})"));
    return Ok(Verdict::Findings);
  }
  std::fs::write(&receipt, format!("{{\"commit\":\"{head}\"}}\n"))
    .map_err(|err| format!("deploy-gate: cannot write {RECEIPT}: {err}"))?;
  output::say(&format!("deploy-gate: ok {head}, receipt {RECEIPT}"));
  Ok(Verdict::Clean)
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
    Stdio::piped(),
    Some(DOCKERFILE),
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

/// `docker run` of the gate in `image` for `head`, with CI's 4 runner CPUs and
/// nextest threads, so timing-sensitive tests see what CI sees.
pub(crate) fn container(image: &str, head: &str) -> Command {
  let mut command = Command::new("docker");
  command
    .args(["run", "--rm", "-i", "--init", "--cpus", "4"])
    .args(["-e", "SRC=/src", "-e", "NEXTEST_TEST_THREADS=4", "-e"])
    .arg(format!("EXPECTED={head}"))
    .args([image, "sh", "-euc"])
    .arg(format!("{CLONE}{SCRIPT}"));
  command
}

/// Run `command` with `stdin`, writing `input` to it when given; output is
/// inherited. A child that exits before reading its input reports its own status.
pub(crate) fn with_input(
  command: &mut Command,
  stdin: Stdio,
  input: Option<&str>,
) -> Result<ExitStatus, String> {
  let mut child = command.stdin(stdin).spawn().map_err(|err| {
    let program = command.get_program().to_string_lossy();
    format!("deploy-gate: cannot run {program}: {err}")
  })?;
  let written = match (input, child.stdin.take()) {
    (Some(text), Some(mut pipe)) => std::io::Write::write_all(&mut pipe, text.as_bytes()),
    _ => Ok(()),
  };
  let status = child.wait().map_err(|err| format!("deploy-gate: {err}"))?;
  match written {
    Err(err) if status.success() => Err(format!("deploy-gate: cannot write the input: {err}")),
    _ => Ok(status),
  }
}

#[cfg(test)]
#[path = "tests/deploy_gate_test.rs"]
mod tests;
