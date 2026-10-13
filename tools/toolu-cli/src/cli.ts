#!/usr/bin/env node
/**
 * `npx @toolu/plugins …`: runs `toolu plugins …` on the separately installed
 * native binary (#438). The package's command is `toolu-plugins`, so it never
 * shadows that binary on `PATH`, and it never takes itself for it.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { catalogPath, findNative } from "./resolve";

const INSTALL = `Install it, then run this again:
  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
  brew install falconiere/tap/toolu
See https://github.com/Falconiere/toolu/blob/main/docs/install.md`;

/** sysexits EX_UNAVAILABLE, the CLI contract's code for a missing dependency. */
const UNAVAILABLE = 69;

function version(self: string): string {
  for (const candidate of ["../package.json", "../npm/package.json"]) {
    const path = resolve(self, "..", candidate);
    if (!existsSync(path)) continue;
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    if (typeof parsed === "object" && parsed !== null && "version" in parsed && typeof parsed.version === "string") {
      return parsed.version;
    }
  }
  return "unknown";
}

function main(argv: readonly string[]): number {
  const self = realpathSync(fileURLToPath(import.meta.url));
  if (argv.length === 1 && (argv[0] === "--version" || argv[0] === "-v")) {
    process.stdout.write(`${version(self)}\n`);
    return 0;
  }
  const native = findNative(process.env, self);
  if (native.kind === "missing") {
    process.stderr.write(`toolu-plugins: the native toolu binary was not found (looked at ${native.searched.join(", ") || "nothing"}).\n${INSTALL}\n`);
    return UNAVAILABLE;
  }
  if (native.kind === "old") {
    process.stderr.write(`toolu-plugins: ${native.path} has no \`toolu plugins\`; it predates this package. Upgrade it:\n  brew upgrade toolu\n  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash\n`);
    return UNAVAILABLE;
  }
  const catalog = catalogPath(self);
  const words = argv.length > 0 && existsSync(catalog) ? ["plugins", "--catalog", catalog, ...argv] : ["plugins", ...argv];
  const result = spawnSync(native.path, words, { stdio: "inherit" });
  if (result.signal !== null) {
    process.kill(process.pid, result.signal);
    return 1;
  }
  return result.status ?? 1;
}

process.exitCode = main(process.argv.slice(2));
