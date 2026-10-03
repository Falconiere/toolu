/**
 * Resolve the pinned OpenCode CLI for the live probes (#335). An explicit
 * `TOOLU_OPENCODE_HOST_BIN` must report the pinned version. Otherwise the CLI
 * is installed from npm (`bun add --exact opencode-ai@<pin>`) into a
 * version-keyed cache directory, never onto PATH or into the user's profile.
 */
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { envOr } from "../env.ts";
import { ContractError, type Pin } from "./schema.ts";

export type HostBinary = { bin: string; version: string; installSource: string };
type Env = Record<string, string | undefined>;

/**
 * A hang guard, not a budget, like `RUN_TIMEOUT_MS`. Under shared CPU load the
 * pinned binary took 34 s, and over 60 s, to print its version and exit (#345),
 * so 15 s killed a healthy host before any scenario ran.
 */
const VERSION_TIMEOUT_MS = 120_000;
const INSTALL_TIMEOUT_MS = 300_000;

/** `<bin> --version`, trimmed; a missing or failing binary is a ContractError. */
async function hostVersion(bin: string, timeoutMs: number): Promise<string> {
  const res = await run([bin, "--version"], { timeoutMs });
  if (res.exitCode !== 0 || res.timedOut) {
    throw new ContractError(
      `cannot run ${bin} --version (exit ${res.exitCode}, timedOut ${res.timedOut}): ${res.stderr.trim()}`,
    );
  }
  return res.stdout.trim().replace(/^v/, "");
}

/** Version-keyed cache root for the pinned CLI and the probes' shared package caches. */
export function hostCacheDir(pin: Pin, env: Env = process.env): string {
  const xdgCache = envOr("XDG_CACHE_HOME", join(homedir(), ".cache"), env);
  return join(
    envOr("TOOLU_OPENCODE_HOST_CACHE", join(xdgCache, "toolu/opencode-host"), env),
    pin.cli.version,
  );
}

async function installPinned(pin: Pin, dir: string): Promise<string> {
  const bin = join(dir, "node_modules/.bin/opencode");
  if (existsSync(bin)) return bin;
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "package.json"),
    `${JSON.stringify({ name: "toolu-opencode-host", private: true })}\n`,
  );
  const spec = `${pin.cli.package}@${pin.cli.version}`;
  const res = await run([process.execPath, "add", "--exact", spec], {
    cwd: dir,
    timeoutMs: INSTALL_TIMEOUT_MS,
  });
  if (res.exitCode !== 0 || !existsSync(bin)) {
    throw new ContractError(
      `bun add --exact ${spec} failed (exit ${res.exitCode}): ${res.stderr.trim()}`,
    );
  }
  return bin;
}

export async function resolveHostBinary(
  pin: Pin,
  env: Env = process.env,
  versionTimeoutMs = VERSION_TIMEOUT_MS,
): Promise<HostBinary> {
  const explicit = env.TOOLU_OPENCODE_HOST_BIN;
  const bin =
    explicit === undefined || explicit === ""
      ? await installPinned(pin, join(hostCacheDir(pin, env), "cli"))
      : explicit;
  const version = await hostVersion(bin, versionTimeoutMs);
  if (version !== pin.cli.version) {
    throw new ContractError(`pin mismatch: ${bin} reports ${version}, pin is ${pin.cli.version}`);
  }
  const installSource =
    explicit === undefined || explicit === ""
      ? `npm:${pin.cli.package}@${pin.cli.version} (bun add --exact)`
      : "TOOLU_OPENCODE_HOST_BIN";
  return { bin, version, installSource };
}
