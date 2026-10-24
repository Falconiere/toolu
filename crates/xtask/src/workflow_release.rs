//! The release half of the workflow contract: release-please runs npm beside
//! the native release, which builds four targets, packages checksums and an
//! SBOM, attests, uploads, finalizes and only then publishes. The native
//! release never waits for npm: the npm package is a wrapper that locates an
//! installed binary, and npm makes a version visible on its own schedule, so
//! npm-publish.yml verifies the registry in a job nothing else needs.

use std::path::Path;

use serde_json::Value;

use crate::workflow_ci::Docs;
use crate::workflow_yaml::{Findings, at, includes, items, lists, runs, step_where, text};

/// release-please's own config and the caller workflow.
fn caller(root: &Path, please: &Value, found: &mut Findings) -> Result<(), String> {
  let path = root.join("release-please-config.json");
  let config: Value = std::fs::read_to_string(&path)
    .map_err(|err| err.to_string())
    .and_then(|text| serde_json::from_str(&text).map_err(|err| err.to_string()))
    .map_err(|err| format!("release-please-config.json: cannot read or parse: {err}"))?;
  let package = at(&config, &["packages", "."]);
  found.need(
    package.get("draft") == Some(&Value::Bool(true))
      && package.get("force-tag-creation") == Some(&Value::Bool(true)),
    "release-please must create a draft and a real tag",
  );
  found.need(
    at(please, &["concurrency", "cancel-in-progress"]) == &Value::Bool(false),
    "release-please.yml must not cancel a release in progress",
  );
  let job = |id: &str, key: &str| at(please, &["jobs", id, key]);
  found.need(
    includes(job("release-please", "if"), "TOOLU_RELEASE_DISABLED"),
    "release-please.yml lacks kill switch",
  );
  found.need(
    text(job("native", "uses")) == "./.github/workflows/release-native.yml",
    "release-please.yml must chain native release",
  );
  let native_needs = job("native", "needs");
  found.need(
    (text(native_needs) == "release-please" || lists(native_needs, "release-please"))
      && !lists(native_needs, "publish")
      && !includes(job("native", "if"), "needs.publish"),
    "release-please.yml native release must not wait for npm",
  );
  found.need(
    text(job("publish", "uses")) == "./.github/workflows/npm-publish.yml"
      && text(job("publish", "secrets")) != "inherit",
    "release-please.yml must chain npm with explicit secrets",
  );
  Ok(())
}

/// Four targets built `--locked`, a gated verify, checksums and an SBOM.
fn build(native: &Value, found: &mut Findings) {
  let jobs = at(native, &["jobs"]);
  for id in [
    "verify", "build", "package", "upload", "finalize", "publish",
  ] {
    found.need(
      jobs.get(id).is_some(),
      &format!("release-native.yml lacks {id}"),
    );
  }
  let targets = items(at(jobs, &["build", "strategy", "matrix", "include"]));
  for (runner, target) in [
    ("macos-14", "aarch64-apple-darwin"),
    ("macos-15-intel", "x86_64-apple-darwin"),
    ("ubuntu-24.04", "x86_64-unknown-linux-musl"),
    ("ubuntu-24.04-arm", "aarch64-unknown-linux-musl"),
  ] {
    found.need(
      targets
        .iter()
        .any(|leg| text(at(leg, &["runner"])) == runner && text(at(leg, &["target"])) == target),
      &format!("release-native.yml lacks {target} on {runner}"),
    );
  }
  let script = |id: &str| runs(at(jobs, &[id]));
  found.need(
    script("build").contains("cargo build --release --locked --bin toolu"),
    "release-native.yml build must use --locked",
  );
  let verify = script("verify");
  found.need(
    [
      "release_native.py verify-tag",
      "cargo xtask gate",
      "python3 -B -m unittest discover -s .github/scripts",
    ]
    .iter()
    .all(|part| verify.contains(part)),
    "release-native.yml verify must test the helper and gate the tag",
  );
  found.need(
    !verify.contains("npm view"),
    "release-native.yml verify must not block on npm",
  );
  package(at(jobs, &["package"]), found);
}

/// The package job writes the checksums and the SPDX SBOM.
fn package(job: &Value, found: &mut Findings) {
  let sbom = step_where(job, |step| {
    includes(at(step, &["uses"]), "sbom-action@")
      && text(at(step, &["with", "output-file"])) == "dist/toolu.spdx.json"
  });
  let script = runs(job);
  found.need(
    script.contains("release_native.py package")
      && script.contains("SHA256SUMS")
      && !sbom.is_null(),
    "release-native.yml must package checksums and SBOM",
  );
}

/// Attest, upload, finalize, then publish; a dry run writes nothing.
fn publish(native: &Value, found: &mut Findings) {
  let job = |id: &str| at(native, &["jobs", id]);
  found.need(
    at(native, &["concurrency", "cancel-in-progress"]) == &Value::Bool(false),
    "release-native.yml must not cancel an in-flight release",
  );
  let finalize_needs = at(job("finalize"), &["needs"]);
  found.need(
    lists(finalize_needs, "package")
      && lists(finalize_needs, "upload")
      && includes(
        at(job("finalize"), &["if"]),
        "needs.upload.result == 'success'",
      ),
    "release-native.yml finalizer must wait for packaged and uploaded assets",
  );
  let upload = job("upload");
  let attest = step_where(upload, |step| {
    includes(at(step, &["uses"]), "attest-build-provenance@")
  });
  found.need(
    !attest.is_null() && runs(upload).contains("gh release upload"),
    "release-native.yml must attest and upload assets",
  );
  let publish_needs = at(job("publish"), &["needs"]);
  found.need(
    lists(publish_needs, "finalize") || text(publish_needs) == "finalize",
    "release-native.yml publish must wait for finalization",
  );
  let script = runs(job("publish"));
  found.need(
    script.contains("--draft=false") && script.contains("--prerelease"),
    "release-native.yml must publish verified stable and prerelease tags",
  );
  found.need(
    includes(at(job("finalize"), &["uses"]), "release-finalize.yml"),
    "release-native.yml must call finalizer",
  );
  let default = at(
    native,
    &["on", "workflow_dispatch", "inputs", "publish", "default"],
  );
  found.need(
    default == &Value::Bool(false)
      && includes(at(job("upload"), &["if"]), "inputs.publish")
      && includes(at(job("publish"), &["if"]), "inputs.publish"),
    "release-native.yml dry run must skip release writes",
  );
}

/// Four smoke-tested archives, and a scheduled advisory audit of the latest tag.
fn finalize(finalize: &Value, audit: &Value, found: &mut Findings) {
  let smoke = at(finalize, &["jobs", "smoke-test"]);
  found.need(
    items(at(smoke, &["strategy", "matrix", "include"])).len() == 4,
    "release-finalize.yml must check four archives",
  );
  let script = runs(smoke);
  found.need(
    script.matches("release_native.py verify-package").count() == 2,
    "release-finalize.yml must verify both published and dry-run archives",
  );
  found.need(
    script.contains("--test-tag") && includes(at(smoke, &["env", "PUBLISHED"]), "inputs.published"),
    "release-finalize.yml must distinguish a dry-run test tag from a published version",
  );
  for required in ["codesign --verify", "alpine:", "debian:", "statically"] {
    found.need(
      script.contains(required),
      &format!("release-finalize.yml lacks {required}"),
    );
  }
  let audited = runs(at(audit, &["jobs", "audit"]));
  found.need(
    !items(at(audit, &["on", "schedule"])).is_empty()
      && audited.contains("cargo deny check advisories")
      && audited.contains("releases/latest"),
    "advisory-audit.yml must audit the latest tag on a schedule",
  );
}

/// A draft is visible only to a write token, so the smoke test runs the run's
/// artifact with no token, and the upload job proves the draft holds those bytes.
fn draft_access(native: &Value, finalize: &Value, found: &mut Findings) {
  let smoke = at(finalize, &["jobs", "smoke-test"]);
  let artifact = step_where(smoke, |step| {
    includes(at(step, &["uses"]), "download-artifact@")
      && text(at(step, &["with", "name"])) == "native-release"
  });
  found.need(
    !artifact.is_null()
      && !runs(smoke).contains("gh release download")
      && at(smoke, &["env", "GH_TOKEN"]).is_null(),
    "release-finalize.yml must smoke-test the run's native-release artifact without a token",
  );
  let upload = runs(at(native, &["jobs", "upload"]));
  found.need(
    upload.contains("gh release download") && upload.contains("cmp "),
    "release-native.yml upload must verify the draft holds the packaged bytes",
  );
}

/// npm publishing never cancels, leaves `latest` alone for a prerelease, and
/// ends when npm accepts the upload; the registry wait is its own job, asking
/// the uncached per-version document.
fn npm(npm: &Value, found: &mut Findings) {
  found.need(
    at(npm, &["concurrency", "cancel-in-progress"]) == &Value::Bool(false),
    "npm-publish.yml must not cancel an in-flight publication",
  );
  let script = runs(at(npm, &["jobs", "publish"]));
  found.need(
    script.contains("npm_tag=next")
      && script.contains("npm publish --provenance --access public --tag \"$npm_tag\""),
    "npm-publish.yml prereleases must leave the latest dist-tag untouched",
  );
  found.need(
    !script.contains("sleep "),
    "npm-publish.yml publish must end when npm accepts the upload",
  );
  let verify = at(npm, &["jobs", "verify"]);
  found.need(
    text(at(verify, &["needs"])) == "publish"
      && runs(verify).contains("https://registry.npmjs.org/${name/\\//%2F}/$pkg"),
    "npm-publish.yml must verify the registry's per-version document in its own job",
  );
  found.need(
    script.contains("npm deprecate")
      && !script.contains("gh release download")
      && !script.contains("tools/toolu-cli/npm/bin/"),
    "npm-publish.yml must publish a deprecation notice and no bundled binaries",
  );
}

/// The whole release chain.
pub(crate) fn release(root: &Path, docs: &Docs<'_>, found: &mut Findings) -> Result<(), String> {
  caller(root, docs.get("release-please.yml"), found)?;
  let native = docs.get("release-native.yml");
  build(native, found);
  publish(native, found);
  finalize(
    docs.get("release-finalize.yml"),
    docs.get("advisory-audit.yml"),
    found,
  );
  draft_access(native, docs.get("release-finalize.yml"), found);
  npm(docs.get("npm-publish.yml"), found);
  Ok(())
}

#[cfg(test)]
#[path = "tests/workflow_release_test.rs"]
mod tests;
