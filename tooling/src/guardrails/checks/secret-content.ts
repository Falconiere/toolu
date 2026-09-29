/**
 * secret-content — a file's CONTENT must never hold a live-looking secret: a
 * private key or an API token pasted into ordinary source ships in files the
 * kit otherwise welcomes tracked. The complement of `secrets`, which bans
 * whole secret files.
 *
 * Semantics kept from the grep implementation: patterns match within one line,
 * the first matching line decides the label, binary files (a NUL byte, grep
 * -I) are skipped, and a file that cannot be read fails the gate closed.
 * scripts/guardrails/ is always exempt: the kit's own fixtures plant fakes.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { git, inGitRepo } from "../git.ts";
import { matchAny, matchGlob } from "../glob.ts";
import { fatal } from "../report.ts";
import { isFile } from "../walk.ts";

const PATTERN =
  /-----BEGIN [A-Z ]*PRIVATE KEY-----|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{22}|xox[baprs]-[0-9A-Za-z-]{10}|sk_live_[0-9A-Za-z]{20}|AIza[0-9A-Za-z_-]{35}/;

/** Which pattern matched, judged on the whole matching line, in the grep-era order. */
const KINDS: ReadonlyArray<readonly [string, string]> = [
  ["*PRIVATE KEY-----*", "private key"],
  ["*AKIA*", "AWS access key id"],
  ["*gh[pousr]_*", "GitHub token"],
  ["*github_pat_*", "GitHub token"],
  ["*xox[baprs]-*", "Slack token"],
  ["*sk_live_*", "Stripe live key"],
  ["*AIza*", "Google API key"],
];

function kindOf(line: string): string {
  return KINDS.find(([glob]) => matchGlob(glob, line))?.[1] ?? "secret";
}

function exempt(config: GuardrailsConfig, path: string): boolean {
  return path.startsWith("scripts/guardrails/") || matchAny(config.secretScanExempt, path);
}

/** The first line holding a secret, or null. Binary content is never scanned. */
function firstSecretLine(root: string, path: string): string | null {
  let bytes: Buffer;
  try {
    bytes = readFileSync(resolve(root, path));
  } catch (err: unknown) {
    const reason = err instanceof Error ? err.message : String(err);
    return fatal(`secret-content scan failed on ${path}: ${reason}`);
  }
  if (bytes.includes(0)) return null;
  // latin1 maps every byte to one char, so ASCII patterns match byte for byte.
  return (
    bytes
      .toString("latin1")
      .split("\n")
      .find((line) => PATTERN.test(line)) ?? null
  );
}

function scan(ctx: CheckContext<GuardrailsConfig>, path: string): void {
  if (exempt(ctx.config, path) || !isFile(ctx.root, path)) return;
  const line = firstSecretLine(ctx.root, path);
  if (line === null) return;
  ctx.report.violation(
    "secret-content",
    path,
    `committed secret value (${kindOf(line)})`,
    "rotate the credential, remove it from this file, and load it from .dev.vars/env instead",
  );
}

export function secretContent(ctx: CheckContext<GuardrailsConfig>, mode: Mode, path: string): void {
  if (mode === "file") {
    scan(ctx, path);
    return;
  }
  if (!inGitRepo(ctx.root)) return;
  // -z: raw bytes, never C-quoted, so café.ts or "-c" cannot bypass the scan.
  const listing = git(ctx.root, ["ls-files", "-z"]);
  if (listing.status !== 0) fatal("secret-content repo scan: git ls-files failed");
  for (const tracked of listing.stdout.split("\0")) {
    if (tracked !== "") scan(ctx, tracked);
  }
}
