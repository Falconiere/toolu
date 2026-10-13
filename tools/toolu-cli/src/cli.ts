#!/usr/bin/env node
/**
 * `npx @toolu/plugins …`: runs `toolu plugins …` on the separately installed
 * native binary (#438). The package's command is `toolu-plugins`, so it never
 * shadows that binary on `PATH`, and it never takes itself for it.
 */
import { spawn } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { constants } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { catalogPath, findNative, nativeWords } from "./resolve";

const INSTALL = `Install it, then run this again:
  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
  brew install falconiere/tap/toolu
See https://github.com/Falconiere/toolu/blob/main/docs/install.md`;

/** sysexits EX_UNAVAILABLE, the CLI contract's code for a missing dependency. */
const UNAVAILABLE = 69;

/** The signals the wrapper hands on, so the native run never outlives it. */
const FORWARDED: readonly NodeJS.Signals[] = ["SIGINT", "SIGTERM", "SIGHUP"];

/** The wrapper's version: `package.json` beside `dist/` (published) or `src/` (a checkout). */
function version(self: string): string {
  const path = resolve(self, "../../package.json");
  if (!existsSync(path)) return "unknown";
  const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
  if (typeof parsed === "object" && parsed !== null && "version" in parsed && typeof parsed.version === "string") {
    return parsed.version;
  }
  return "unknown";
}

/**
 * Runs the native binary with `words` and exits with its status; a run killed
 * by a signal exits 128 plus the signal's number, as a shell reports it.
 */
function runNative(path: string, words: readonly string[]): void {
  const child = spawn(path, [...words], { stdio: "inherit" });
  const forward = (signal: NodeJS.Signals) => () => child.kill(signal);
  for (const signal of FORWARDED) process.on(signal, forward(signal));
  child.on("error", (error) => {
    process.stderr.write(`toolu-plugins: cannot run ${path}: ${error.message}\n`);
    process.exitCode = UNAVAILABLE;
  });
  child.on("exit", (code, signal) => {
    process.exitCode = signal === null ? (code ?? 1) : 128 + (constants.signals[signal] ?? 0);
  });
}

function main(argv: readonly string[]): void {
  const self = realpathSync(fileURLToPath(import.meta.url));
  if (argv.length === 1 && (argv[0] === "--version" || argv[0] === "-v")) {
    process.stdout.write(`${version(self)}\n`);
    return;
  }
  const native = findNative(process.env, self);
  if (native.kind === "missing") {
    const looked = native.searched.join(", ") || "nothing";
    process.stderr.write(`toolu-plugins: the native toolu binary was not found (looked at ${looked}).\n${INSTALL}\n`);
    process.exitCode = UNAVAILABLE;
    return;
  }
  if (native.kind === "old") {
    process.stderr.write(
      `toolu-plugins: ${native.path} has no \`toolu plugins\`; it predates this package. Upgrade it:\n  brew upgrade toolu\n  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash\n`,
    );
    process.exitCode = UNAVAILABLE;
    return;
  }
  const catalog = catalogPath(self);
  runNative(native.path, nativeWords(argv, existsSync(catalog) ? catalog : undefined));
}

main(process.argv.slice(2));
