#!/usr/bin/env node
// `npx @toolu/plugins …`: runs `toolu plugins …`, preferring the native
// binary the package ships (`bin/<os>-<arch>/toolu`) and falling back to a
// separately installed one (#438, bundled binaries). The package's command is
// `toolu-plugins`, so it never shadows that binary on `PATH`, and it never
// takes itself for it. Plain, dependency-free JavaScript: there is no build
// step (#440).
import { spawn, spawnSync } from "node:child_process";
import { accessSync, constants, existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import { constants as os } from "node:os";
import { delimiter, dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const INSTALL = `Install it, then run this again:
  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
  brew install falconiere/tap/toolu
See https://github.com/Falconiere/toolu/blob/main/docs/install.md`;

/** sysexits EX_UNAVAILABLE, the CLI contract's code for a missing dependency. */
const UNAVAILABLE = 69;

/** The signals the wrapper hands on, so the native run never outlives it. */
const FORWARDED = ["SIGINT", "SIGTERM", "SIGHUP"];

/** The install directories the plugins' hook launcher searches (`crates/core/protocol/src/launcher.rs`). */
const FIXED_DIRS = ["/opt/homebrew/bin", "/usr/local/bin", "/home/linuxbrew/.linuxbrew/bin"];

/** How long one probe of a candidate may take. */
const PROBE_TIMEOUT_MS = 20_000;

/** The verbs that take `--catalog`. */
const VERBS = new Set(["install", "list", "remove", "update"]);

/**
 * Where the native `toolu` may live, the way the hook launcher searches: a
 * non-empty `TOOLU_BIN` alone, else the install directories, `~/.local/bin`,
 * then every `toolu` on `PATH`.
 */
function candidates(env) {
  const bin = env.TOOLU_BIN ?? "";
  if (bin !== "") return [bin];
  const home = env.HOME ?? "";
  const dirs = [...FIXED_DIRS, ...(home === "" ? [] : [join(home, ".local/bin")])];
  const onPath = (env.PATH ?? "").split(delimiter).filter((dir) => dir !== "");
  return [...dirs, ...onPath].map((dir) => join(dir, "toolu"));
}

function real(path) {
  try {
    return realpathSync(path);
  } catch {
    return undefined;
  }
}

function executableFile(path) {
  try {
    if (!statSync(path).isFile()) return false;
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/** This file, a copy inside its own package folder, or a global install of the package. */
function isWrapper(path, self) {
  return (
    path === self ||
    path.startsWith(`${dirname(self)}${sep}`) ||
    path.includes(`${sep}node_modules${sep}@toolu${sep}plugins${sep}`)
  );
}

function answers(path, args, env) {
  const result = spawnSync(path, args, {
    encoding: "utf8",
    env,
    timeout: PROBE_TIMEOUT_MS,
    stdio: ["ignore", "pipe", "ignore"],
  });
  return result.status === 0 ? result.stdout : undefined;
}

/** Whether `path` answers the native probes: `found`, `old`, or no answer. */
function probeCandidate(path, env) {
  const protocol = answers(path, ["--hook-protocol"], env);
  if (protocol === undefined || !/^\d+\s*$/.test(protocol)) return undefined;
  // clap's help always has a usage line; an empty or foreign answer is no `plugins install`.
  const help = answers(path, ["plugins", "install", "--help"], env);
  if (help === undefined || !help.includes("Usage: toolu plugins install")) return "old";
  return "found";
}

/** The bundled binary's directory for this platform, or undefined elsewhere. */
function platformArch() {
  const platform = process.platform;
  if (platform !== "darwin" && platform !== "linux") return undefined;
  const arch = process.arch;
  if (arch !== "arm64" && arch !== "x64") return undefined;
  return `${platform}-${arch === "arm64" ? "arm64" : "amd64"}`;
}

/** The binary shipped beside this file for this platform, or undefined. */
function bundledPath(self) {
  const dir = platformArch();
  return dir === undefined ? undefined : join(dirname(self), "bin", dir, "toolu");
}

/**
 * The bundled binary, probed like any native binary except the
 * inside-the-package exclusion, which would always reject it. Returns
 * `{ kind: "found", path }`, `{ kind: "unusable", path }`, or undefined when
 * this platform ships nothing or the file is absent. Callers fall through on
 * anything but found, so a corrupt bundle never runs and never blocks the
 * system search.
 */
function probeBundled(env, self) {
  const candidate = bundledPath(self);
  if (candidate === undefined) return undefined;
  const path = real(candidate);
  if (path === undefined || !executableFile(path)) return undefined;
  const verdict = probeCandidate(path, env);
  return verdict === "found" ? { kind: "found", path } : { kind: "unusable", path };
}

/**
 * The native binary: the first candidate that is an executable file, is not
 * this wrapper, prints an integer for `--hook-protocol` and has `plugins
 * install`. Each real path is probed once. `kind` is `found`, `old` (a
 * native binary without `toolu plugins`) or `missing`.
 */
function findNative(env, self) {
  const searched = candidates(env);
  const tried = new Set();
  let old;
  for (const candidate of searched) {
    const path = real(candidate);
    if (path === undefined || tried.has(path)) continue;
    tried.add(path);
    if (isWrapper(path, self) || !executableFile(path)) continue;
    const verdict = probeCandidate(path, env);
    if (verdict === undefined) continue;
    if (verdict === "found") return { kind: "found", path };
    old ??= path;
  }
  return old === undefined ? { kind: "missing", searched } : { kind: "old", path: old };
}

/** The catalog shipped beside this file, else the repository's when run from a checkout. */
function catalogPath(self) {
  const here = dirname(self);
  const shipped = join(here, "assets/marketplace.json");
  if (existsSync(shipped)) return shipped;
  const checkout = resolve(here, "../../../.claude-plugin/marketplace.json");
  return existsSync(checkout) ? checkout : undefined;
}

/**
 * The words after `toolu`: `plugins`, then the user's, with `--catalog <catalog>`
 * after the first known verb unless the user gave their own `--catalog`.
 */
function nativeWords(argv, catalog) {
  const own = argv.some((word) => word === "--catalog" || word.startsWith("--catalog="));
  const at = argv.findIndex((word) => VERBS.has(word));
  if (catalog === undefined || own || at < 0) return ["plugins", ...argv];
  return ["plugins", ...argv.slice(0, at + 1), "--catalog", catalog, ...argv.slice(at + 1)];
}

/** The wrapper's version, from the `package.json` beside it. */
function version(self) {
  const path = join(dirname(self), "package.json");
  if (!existsSync(path)) return "unknown";
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  return typeof parsed?.version === "string" ? parsed.version : "unknown";
}

/**
 * Runs the native binary with `words` and exits with its status; a run killed
 * by a signal exits 128 plus the signal's number, as a shell reports it.
 */
function runNative(path, words) {
  const child = spawn(path, words, { stdio: "inherit", windowsHide: true });
  for (const signal of FORWARDED) process.on(signal, () => child.kill(signal));
  child.on("error", (error) => {
    process.stderr.write(`toolu-plugins: cannot run ${path}: ${error.message}\n`);
    process.exitCode = UNAVAILABLE;
  });
  child.on("exit", (code, signal) => {
    process.exitCode = signal === null ? (code ?? 1) : 128 + (os.signals[signal] ?? 0);
  });
}

function main(argv) {
  const self = realpathSync(fileURLToPath(import.meta.url));
  if (argv.length === 1 && (argv[0] === "--version" || argv[0] === "-v")) {
    process.stdout.write(`${version(self)}\n`);
    return;
  }
  // An explicit TOOLU_BIN is the only candidate; otherwise the bundled
  // binary wins over the system search, and anything but a working bundle
  // falls through to it.
  if ((process.env.TOOLU_BIN ?? "") === "") {
    const bundled = probeBundled(process.env, self);
    if (bundled !== undefined && bundled.kind === "found") {
      runNative(bundled.path, nativeWords(argv, catalogPath(self)));
      return;
    }
  }
  const native = findNative(process.env, self);
  if (native.kind === "missing") {
    const looked = native.searched.join(", ") || "nothing";
    process.stderr.write(
      `toolu-plugins: the native toolu binary was not found (looked at ${looked}).\n${INSTALL}\n`,
    );
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
  runNative(native.path, nativeWords(argv, catalogPath(self)));
}

main(process.argv.slice(2));
