/** Shared YAML accessors and native release workflow checks. */
import { readFileSync } from "node:fs";
import { join } from "node:path";

export type ObjectMap = Record<string, unknown>;

export function object(value: unknown): ObjectMap {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value))
    : {};
}

export function at(value: unknown, ...keys: string[]): unknown {
  return keys.reduce<unknown>((current, key) => object(current)[key], value);
}

export function string(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function array(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

export function steps(job: unknown): ObjectMap[] {
  return array(at(job, "steps")).map(object);
}

export function runs(job: unknown): string {
  return steps(job)
    .map((step) => string(step.run))
    .join("\n");
}

export function includes(value: unknown, needle: string): boolean {
  return string(value).includes(needle);
}

export function need(errors: string[], condition: boolean, message: string): void {
  if (!condition) errors.push(message);
}

function checkReleaseCaller(root: string, caller: ObjectMap, errors: string[]): void {
  const config: unknown = JSON.parse(
    readFileSync(join(root, "release-please-config.json"), "utf8"),
  );
  need(
    errors,
    at(config, "packages", ".", "draft") === true &&
      at(config, "packages", ".", "force-tag-creation") === true,
    "release-please must create a draft and a real tag",
  );
  need(
    errors,
    at(caller, "concurrency", "cancel-in-progress") === false,
    "release-please.yml must not cancel a release in progress",
  );
  need(
    errors,
    includes(at(caller, "jobs", "release-please", "if"), "TOOLU_RELEASE_DISABLED"),
    "release-please.yml lacks kill switch",
  );
  need(
    errors,
    at(caller, "jobs", "native", "uses") === "./.github/workflows/release-native.yml",
    "release-please.yml must chain native release",
  );
  need(
    errors,
    array(at(caller, "jobs", "native", "needs")).includes("publish"),
    "release-please.yml native release must wait for npm availability",
  );
  need(
    errors,
    at(caller, "jobs", "publish", "uses") === "./.github/workflows/npm-publish.yml" &&
      at(caller, "jobs", "publish", "secrets") !== "inherit",
    "release-please.yml must chain npm with explicit secrets",
  );
}

function checkNativeBuild(native: ObjectMap, errors: string[]): void {
  const jobs = object(native.jobs);
  for (const id of ["verify", "build", "package", "upload", "finalize", "publish"]) {
    need(errors, jobs[id] !== undefined, `release-native.yml lacks ${id}`);
  }
  const targets = array(at(jobs, "build", "strategy", "matrix", "include"));
  for (const [runner, target] of [
    ["macos-14", "aarch64-apple-darwin"],
    ["macos-15-intel", "x86_64-apple-darwin"],
    ["ubuntu-24.04", "x86_64-unknown-linux-musl"],
    ["ubuntu-24.04-arm", "aarch64-unknown-linux-musl"],
  ] as const) {
    need(
      errors,
      targets.some((leg) => at(leg, "runner") === runner && at(leg, "target") === target),
      `release-native.yml lacks ${target} on ${runner}`,
    );
  }
  need(
    errors,
    runs(jobs.build).includes("cargo build --release --locked --bin toolu"),
    "release-native.yml build must use --locked",
  );
  need(
    errors,
    runs(jobs.verify).includes("release_native.py verify-tag") &&
      runs(jobs.verify).includes("cargo xtask gate") &&
      runs(jobs.verify).includes("npm view") &&
      runs(jobs.verify).includes("python3 -B -m unittest discover -s .github/scripts"),
    "release-native.yml verify must test the helper, gate the tag, and require npm packages",
  );
  need(
    errors,
    runs(jobs.package).includes("release_native.py package") &&
      runs(jobs.package).includes("SHA256SUMS") &&
      steps(jobs.package).some(
        (step) =>
          includes(step.uses, "sbom-action@") &&
          at(step, "with", "output-file") === "dist/toolu.spdx.json",
      ),
    "release-native.yml must package checksums and SBOM",
  );
}

function checkNativePublish(native: ObjectMap, errors: string[]): void {
  const jobs = object(native.jobs);
  need(
    errors,
    at(native, "concurrency", "cancel-in-progress") === false,
    "release-native.yml must not cancel an in-flight release",
  );
  const finalizeNeeds = array(at(jobs, "finalize", "needs"));
  need(
    errors,
    finalizeNeeds.includes("package") &&
      finalizeNeeds.includes("upload") &&
      includes(at(jobs, "finalize", "if"), "needs.upload.result == 'success'"),
    "release-native.yml finalizer must wait for packaged and uploaded assets",
  );
  need(
    errors,
    steps(jobs.upload).some((step) => includes(step.uses, "attest-build-provenance@")) &&
      runs(jobs.upload).includes("gh release upload"),
    "release-native.yml must attest and upload assets",
  );
  need(
    errors,
    array(at(jobs, "publish", "needs")).includes("finalize") ||
      at(jobs, "publish", "needs") === "finalize",
    "release-native.yml publish must wait for finalization",
  );
  need(
    errors,
    runs(jobs.publish).includes("--draft=false") && runs(jobs.publish).includes("--prerelease"),
    "release-native.yml must publish verified stable and prerelease tags",
  );
  need(
    errors,
    includes(at(jobs, "finalize", "uses"), "release-finalize.yml"),
    "release-native.yml must call finalizer",
  );
  need(
    errors,
    at(native, "on", "workflow_dispatch") !== undefined &&
      at(native, "on", "workflow_dispatch", "inputs", "publish", "default") === false &&
      includes(at(jobs, "upload", "if"), "inputs.publish") &&
      includes(at(jobs, "publish", "if"), "inputs.publish"),
    "release-native.yml dry run must skip release writes",
  );
}

function checkFinalize(finalize: ObjectMap, audit: ObjectMap, errors: string[]): void {
  const smoke = object(at(finalize, "jobs", "smoke-test"));
  need(
    errors,
    array(at(smoke, "strategy", "matrix", "include")).length === 4,
    "release-finalize.yml must check four archives",
  );
  const smokeRuns = runs(smoke);
  need(
    errors,
    (smokeRuns.match(/release_native\.py verify-package/g) ?? []).length === 2,
    "release-finalize.yml must verify both published and dry-run archives",
  );
  need(
    errors,
    smokeRuns.includes("--test-tag") && includes(at(smoke, "env", "PUBLISHED"), "inputs.published"),
    "release-finalize.yml must distinguish a dry-run test tag from a published version",
  );
  for (const required of ["codesign --verify", "alpine:", "debian:", "statically"]) {
    need(errors, smokeRuns.includes(required), `release-finalize.yml lacks ${required}`);
  }
  need(
    errors,
    array(at(audit, "on", "schedule")).length > 0 &&
      runs(at(audit, "jobs", "audit")).includes("cargo deny check advisories") &&
      runs(at(audit, "jobs", "audit")).includes("releases/latest"),
    "advisory-audit.yml must audit the latest tag on a schedule",
  );
}

export function checkRelease(
  root: string,
  docs: ReadonlyMap<string, ObjectMap>,
  errors: string[],
): void {
  const get = (name: string): ObjectMap => docs.get(name) ?? {};
  checkReleaseCaller(root, get("release-please.yml"), errors);
  checkNativeBuild(get("release-native.yml"), errors);
  checkNativePublish(get("release-native.yml"), errors);
  checkFinalize(get("release-finalize.yml"), get("advisory-audit.yml"), errors);
  need(
    errors,
    at(get("npm-publish.yml"), "concurrency", "cancel-in-progress") === false,
    "npm-publish.yml must not cancel an in-flight publication",
  );
  need(
    errors,
    runs(at(get("npm-publish.yml"), "jobs", "publish")).includes("npm_tag=next") &&
      runs(at(get("npm-publish.yml"), "jobs", "publish")).includes(
        'npm publish --provenance --access public --tag "$npm_tag"',
      ),
    "npm-publish.yml prereleases must leave the latest dist-tag untouched",
  );
}
