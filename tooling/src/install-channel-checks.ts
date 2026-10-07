/** Release-chain checks for the install channels (#457): signed sums and the Homebrew tap. */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  type ObjectMap,
  array,
  at,
  includes,
  need,
  runs,
  steps,
  string,
} from "./workflow-checks.ts";

/** The digest `install.sh` pins for minisign's linux archive, or "" without one. */
function installerMinisignDigest(root: string): string {
  const path = join(root, "install.sh");
  if (!existsSync(path)) return "";
  const match = /^MINISIGN_LINUX_SHA256="([0-9a-f]{64})"$/m.exec(readFileSync(path, "utf8"));
  return match?.[1] ?? "";
}

function checkSigning(root: string, native: ObjectMap, errors: string[]): void {
  const jobs = at(native, "jobs");
  const digest = installerMinisignDigest(root);
  need(errors, digest !== "", "install.sh must pin the minisign linux archive digest");
  const sign = steps(at(jobs, "package")).find((step) => step.name === "Sign SHA256SUMS");
  need(
    errors,
    digest !== "" && at(sign, "env", "MINISIGN_SHA256") === digest,
    "release-native.yml must sign with the minisign archive install.sh pins",
  );
  need(
    errors,
    includes(at(sign, "env", "MINISIGN_SECRET_KEY"), "secrets.TOOLU_MINISIGN_SECRET_KEY") &&
      includes(sign?.run, "-x dist/SHA256SUMS.minisig") &&
      includes(sign?.run, "TOOLU_PUBLIC_KEY") &&
      at(native, "on", "workflow_call", "secrets", "TOOLU_MINISIGN_SECRET_KEY", "required") ===
        true,
    "release-native.yml must sign SHA256SUMS with the release key and verify the install.sh key",
  );
  need(
    errors,
    runs(at(jobs, "upload")).includes("dist/SHA256SUMS.minisig"),
    "release-native.yml must upload SHA256SUMS.minisig",
  );
}

function checkCaller(caller: ObjectMap, errors: string[]): void {
  const native = at(caller, "jobs", "native");
  need(
    errors,
    includes(
      at(native, "secrets", "TOOLU_MINISIGN_SECRET_KEY"),
      "secrets.TOOLU_MINISIGN_SECRET_KEY",
    ),
    "release-please.yml must pass TOOLU_MINISIGN_SECRET_KEY to the native release explicitly",
  );
  const homebrew = at(caller, "jobs", "homebrew");
  need(
    errors,
    at(homebrew, "uses") === "./.github/workflows/release-homebrew.yml" &&
      array(at(homebrew, "needs")).includes("native") &&
      includes(at(homebrew, "if"), "needs.native.result == 'success'"),
    "release-please.yml must update the tap only after the native release succeeds",
  );
  need(
    errors,
    includes(at(homebrew, "secrets", "HOMEBREW_APP_ID"), "secrets.HOMEBREW_APP_ID") &&
      includes(
        at(homebrew, "secrets", "HOMEBREW_APP_PRIVATE_KEY"),
        "secrets.HOMEBREW_APP_PRIVATE_KEY",
      ),
    "release-please.yml must pass the Homebrew App secrets explicitly",
  );
}

function checkTap(root: string, tap: ObjectMap, errors: string[]): void {
  const job = at(tap, "jobs", "formula");
  const verify = steps(job).find((step) => step.name === "Verify the SHA256SUMS signature");
  const digest = installerMinisignDigest(root);
  need(
    errors,
    digest !== "" &&
      at(verify, "env", "MINISIGN_SHA256") === digest &&
      includes(verify?.run, "TOOLU_PUBLIC_KEY") &&
      includes(verify?.run, "Trusted comment: toolu $TAG") &&
      runs(job).includes("--pattern SHA256SUMS.minisig"),
    "release-homebrew.yml must verify the SHA256SUMS signature for the tag before the formula",
  );
  need(
    errors,
    at(tap, "on", "workflow_dispatch", "inputs", "tag", "required") === true,
    "release-homebrew.yml must allow a manual run for a tag",
  );
  const condition = string(at(job, "if"));
  need(
    errors,
    condition.includes("!contains(inputs.tag, '-')") &&
      condition.includes("inputs.tag != ''") &&
      !condition.includes("github.ref_name"),
    "release-homebrew.yml must skip prerelease and empty tags by inputs.tag",
  );
  const token = steps(job).find((step) => includes(step.uses, "create-github-app-token@"));
  need(
    errors,
    at(token, "with", "repositories") === "homebrew-tap" &&
      at(tap, "on", "workflow_call", "inputs", "tag", "required") === true,
    "release-homebrew.yml must mint a token scoped to homebrew-tap",
  );
  need(
    errors,
    runs(job).includes('cargo xtask homebrew-formula "$TAG" dist/SHA256SUMS') &&
      runs(job).includes("Formula/toolu.rb"),
    "release-homebrew.yml must generate Formula/toolu.rb from the release SHA256SUMS",
  );
}

export function checkInstallChannels(
  root: string,
  docs: ReadonlyMap<string, ObjectMap>,
  errors: string[],
): void {
  const get = (name: string): ObjectMap => docs.get(name) ?? {};
  checkSigning(root, get("release-native.yml"), errors);
  checkCaller(get("release-please.yml"), errors);
  checkTap(root, get("release-homebrew.yml"), errors);
}
