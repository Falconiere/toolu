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

/// The `verify` job's tools at `release-native.yml`'s pins; a test keeps them equal.
/// Non-root uid 501 and `/src` avoid the permission and `/work` traps of a root gate.
pub(crate) const DOCKERFILE: &str = r#"ARG TOOLCHAIN
FROM node:22-bookworm-slim AS node
FROM rust:${TOOLCHAIN}-bookworm
ARG TOOLCHAIN
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
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
/// `HEAD` other than `$EXPECTED`.
pub(crate) const CLONE: &str = r#"B=$(mktemp)
cat > "$B"
git clone -q "$B" "$SRC"
cd "$SRC"
git fetch -q "$B" refs/remotes/origin/main:refs/remotes/origin/main
test "$(git rev-parse HEAD)" = "$EXPECTED"
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
  let head = preconditions(root)?;
  docker(
    Command::new("docker")
      .args(["build", "-t", IMAGE, "--build-arg"])
      .arg(format!("TOOLCHAIN={}", channel(root)?))
      .arg("-"),
    Stdio::piped(),
    Some(DOCKERFILE),
  )
  .and_then(|status| exited(status, "docker build"))?;
  let bundle = std::fs::File::open(bundle(root)?).map_err(|err| format!("deploy-gate: {err}"))?;
  let status = docker(
    Command::new("docker")
      .args([
        "run",
        "--rm",
        "-i",
        "--init",
        "-e",
        "SRC=/src",
        "-e",
        "NEXTEST_TEST_THREADS=1",
      ])
      .args(["-e", &format!("EXPECTED={head}"), IMAGE, "sh", "-euc"])
      .arg(format!("{CLONE}{SCRIPT}")),
    Stdio::from(bundle),
    None,
  )?;
  if !status.success() {
    output::error(&format!("deploy-gate: container gate failed ({status})"));
    return Ok(Verdict::Findings);
  }
  std::fs::write(&receipt, format!("{{\"commit\":\"{head}\"}}\n"))
    .map_err(|err| format!("deploy-gate: cannot write {RECEIPT}: {err}"))?;
  output::say(&format!("deploy-gate: ok {head}, receipt {RECEIPT}"));
  Ok(Verdict::Clean)
}

/// The `HEAD` to gate: the worktree must be clean and `origin/main` known.
fn preconditions(root: &Path) -> Result<String, String> {
  if !git(root, &["status", "--porcelain"])?.trim().is_empty() {
    return Err("deploy-gate: the worktree is dirty; commit first".to_owned());
  }
  git(
    root,
    &["rev-parse", "-q", "--verify", "refs/remotes/origin/main"],
  )
  .map_err(|err| {
    format!("deploy-gate: refs/remotes/origin/main is missing; run git fetch origin main ({err})")
  })?;
  Ok(git(root, &["rev-parse", "HEAD"])?.trim().to_owned())
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

/// Bundle `HEAD` and `origin/main` into `target/deploy/head.bundle`.
pub(crate) fn bundle(root: &Path) -> Result<PathBuf, String> {
  let path = root.join("target/deploy/head.bundle");
  let dir = root.join("target/deploy");
  std::fs::create_dir_all(&dir).map_err(|err| format!("deploy-gate: {}: {err}", dir.display()))?;
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
  Ok(path)
}

/// Run `command` with `stdin`, writing `input` to it when given; output is inherited.
fn docker(command: &mut Command, stdin: Stdio, input: Option<&str>) -> Result<ExitStatus, String> {
  let mut child = command
    .stdin(stdin)
    .spawn()
    .map_err(|err| format!("deploy-gate: cannot run docker: {err}"))?;
  if let (Some(text), Some(mut pipe)) = (input, child.stdin.take()) {
    std::io::Write::write_all(&mut pipe, text.as_bytes())
      .map_err(|err| format!("deploy-gate: cannot write to docker: {err}"))?;
  }
  child
    .wait()
    .map_err(|err| format!("deploy-gate: docker: {err}"))
}

fn exited(status: ExitStatus, what: &str) -> Result<(), String> {
  if status.success() {
    Ok(())
  } else {
    Err(format!("deploy-gate: {what} failed ({status})"))
  }
}

#[cfg(test)]
#[path = "tests/deploy_gate_test.rs"]
mod tests;
