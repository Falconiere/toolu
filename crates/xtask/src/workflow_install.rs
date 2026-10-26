//! The install-channel half of the workflow contract (#457): `SHA256SUMS` is
//! signed with the key `install.sh` trusts, and the Homebrew tap is updated
//! only from a verified stable release.

use std::path::Path;

use regex::Regex;
use serde_json::Value;

use crate::workflow_ci::Docs;
use crate::workflow_yaml::{Findings, at, includes, lists, runs, step_where, text};

/// The digest `install.sh` pins for minisign's Linux archive, or `""` without one.
fn minisign_digest(root: &Path) -> String {
  let Ok(script) = std::fs::read_to_string(root.join("install.sh")) else {
    return String::new();
  };
  Regex::new(r#"(?m)^MINISIGN_LINUX_SHA256="([0-9a-f]{64})"$"#)
    .ok()
    .and_then(|expr| expr.captures(&script))
    .and_then(|caps| caps.get(1))
    .map_or_else(String::new, |found| found.as_str().to_owned())
}

/// The step of `job` named `name`, else `null`.
fn named<'v>(job: &'v Value, name: &str) -> &'v Value {
  step_where(job, |step| text(at(step, &["name"])) == name)
}

fn signing(native: &Value, digest: &str, found: &mut Findings) {
  found.need(
    !digest.is_empty(),
    "install.sh must pin the minisign linux archive digest",
  );
  let sign = named(at(native, &["jobs", "package"]), "Sign SHA256SUMS");
  found.need(
    !digest.is_empty() && text(at(sign, &["env", "MINISIGN_SHA256"])) == digest,
    "release-native.yml must sign with the minisign archive install.sh pins",
  );
  let script = at(sign, &["run"]);
  let secret = &[
    "on",
    "workflow_call",
    "secrets",
    "TOOLU_MINISIGN_SECRET_KEY",
    "required",
  ];
  found.need(
    includes(
      at(sign, &["env", "MINISIGN_SECRET_KEY"]),
      "secrets.TOOLU_MINISIGN_SECRET_KEY",
    ) && includes(script, "-x dist/SHA256SUMS.minisig")
      && includes(script, "TOOLU_PUBLIC_KEY")
      && at(native, secret) == &Value::Bool(true),
    "release-native.yml must sign SHA256SUMS with the release key and verify the install.sh key",
  );
  let upload = runs(at(native, &["jobs", "upload"]));
  found.need(
    upload.contains("dist/SHA256SUMS.minisig"),
    "release-native.yml must upload SHA256SUMS.minisig",
  );
  found.need(
    upload.contains("dist/install.sh"),
    "release-native.yml must upload install.sh, which get.toolu.sh serves from the latest release",
  );
}

fn caller(please: &Value, found: &mut Findings) {
  let native = at(please, &["jobs", "native"]);
  found.need(
    includes(
      at(native, &["secrets", "TOOLU_MINISIGN_SECRET_KEY"]),
      "secrets.TOOLU_MINISIGN_SECRET_KEY",
    ),
    "release-please.yml must pass TOOLU_MINISIGN_SECRET_KEY to the native release explicitly",
  );
  let homebrew = at(please, &["jobs", "homebrew"]);
  found.need(
    text(at(homebrew, &["uses"])) == "./.github/workflows/release-homebrew.yml"
      && lists(at(homebrew, &["needs"]), "native")
      && includes(at(homebrew, &["if"]), "needs.native.result == 'success'"),
    "release-please.yml must update the tap only after the native release succeeds",
  );
  let secret = |key: &str| includes(at(homebrew, &["secrets", key]), &format!("secrets.{key}"));
  found.need(
    secret("HOMEBREW_APP_ID") && secret("HOMEBREW_APP_PRIVATE_KEY"),
    "release-please.yml must pass the Homebrew App secrets explicitly",
  );
}

fn tap(tap: &Value, digest: &str, found: &mut Findings) {
  let job = at(tap, &["jobs", "formula"]);
  let verify = named(job, "Verify the SHA256SUMS signature");
  let script = runs(job);
  found.need(
    !digest.is_empty()
      && text(at(verify, &["env", "MINISIGN_SHA256"])) == digest
      && includes(at(verify, &["run"]), "TOOLU_PUBLIC_KEY")
      && includes(at(verify, &["run"]), "Trusted comment: toolu $TAG")
      && script.contains("--pattern SHA256SUMS.minisig"),
    "release-homebrew.yml must verify the SHA256SUMS signature for the tag before the formula",
  );
  let required =
    |trigger: &str| at(tap, &["on", trigger, "inputs", "tag", "required"]) == &Value::Bool(true);
  found.need(
    required("workflow_dispatch"),
    "release-homebrew.yml must allow a manual run for a tag",
  );
  let condition = text(at(job, &["if"]));
  found.need(
    condition.contains("!contains(inputs.tag, '-')")
      && condition.contains("inputs.tag != ''")
      && !condition.contains("github.ref_name"),
    "release-homebrew.yml must skip prerelease and empty tags by inputs.tag",
  );
  let token = step_where(job, |step| {
    includes(at(step, &["uses"]), "create-github-app-token@")
  });
  found.need(
    text(at(token, &["with", "repositories"])) == "homebrew-tap" && required("workflow_call"),
    "release-homebrew.yml must mint a token scoped to homebrew-tap",
  );
  found.need(
    script.contains("cargo xtask homebrew-formula \"$TAG\" dist/SHA256SUMS")
      && script.contains("Formula/toolu.rb"),
    "release-homebrew.yml must generate Formula/toolu.rb from the release SHA256SUMS",
  );
}

/// Signing, the release-please secrets, and the tap.
pub(crate) fn install_channels(root: &Path, docs: &Docs<'_>, found: &mut Findings) {
  let digest = minisign_digest(root);
  signing(docs.get("release-native.yml"), &digest, found);
  caller(docs.get("release-please.yml"), found);
  tap(docs.get("release-homebrew.yml"), &digest, found);
}

#[cfg(test)]
#[path = "tests/workflow_install_test.rs"]
mod tests;
