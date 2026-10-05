/**
 * Live CLI management scenarios (#360) on the pinned host.
 *
 * The built `toolu` bundle runs under Node against an isolated profile and
 * edits OpenCode's documented config files; the host then reports what it
 * merged (`debug config`), discovered (`debug skill`) and enforced (a scripted
 * session writing a protected `.env`). The package spec points at the packed
 * tarball through `TOOLU_OPENCODE_PACKAGE`.
 */
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { catalogIds, host, named, skills } from "./install-host.ts";
import {
  GATED_FILES,
  ROOT,
  enforced,
  entrySession,
  npmSpec,
  protectedWrite,
  writeEnvScript,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import { ContractError } from "./schema.ts";
import { PROBE_PLUGIN, type ProbeSession } from "./session.ts";

const CLI_DIR = join(ROOT, "tools/toolu-cli");
const CLI_TIMEOUT_MS = 120_000;
const PACKAGE = "@toolu/opencode";
const ResolvedPlugins = z.looseObject({ plugin: z.array(z.unknown()).optional() });

let built: Promise<string> | undefined;

/** The published layout: `dist/cli.js` beside `assets/marketplace.json` and `package.json`. */
async function buildCli(workDir: string): Promise<string> {
  const out = join(workDir, "toolu-cli");
  mkdirSync(join(out, "assets"), { recursive: true });
  copyFileSync(join(CLI_DIR, "npm/package.json"), join(out, "package.json"));
  copyFileSync(join(ROOT, ".claude-plugin/marketplace.json"), join(out, "assets/marketplace.json"));
  const cli = join(out, "dist/cli.js");
  const res = await run(
    [process.execPath, "build", "src/cli.ts", "--target=node", `--outfile=${cli}`],
    { cwd: CLI_DIR, timeoutMs: CLI_TIMEOUT_MS },
  );
  if (res.exitCode !== 0)
    throw new ContractError(`bun build of the CLI failed (exit ${res.exitCode}): ${res.stderr}`);
  return cli;
}

/** The CLI bundle, built once per run beside the packed tarball. */
export function cliBundle(ctx: EntryContext): Promise<string> {
  built ??= buildCli(dirname(ctx.tarball));
  return built;
}

/** `node dist/cli.js <args> --host opencode` in the project, installing `spec`. */
async function toolu(
  ctx: EntryContext,
  s: ProbeSession,
  args: readonly string[],
  spec = npmSpec(ctx.tarball),
): Promise<number> {
  const node = Bun.which("node");
  if (node === null) throw new ContractError("node is not on PATH; the CLI bundle targets Node");
  const res = await run([node, await cliBundle(ctx), ...args, "--host", "opencode"], {
    cwd: s.sb.project,
    env: { ...s.env, TOOLU_OPENCODE_PACKAGE: spec },
    stdin: "",
    timeoutMs: CLI_TIMEOUT_MS,
  });
  if (res.timedOut || res.exitCode !== 0)
    throw new ContractError(
      `toolu ${args.join(" ")} failed (exit ${res.exitCode}): ${res.stderr.slice(-2000)}${res.stdout.slice(-2000)}`,
    );
  return res.exitCode;
}

function globalDir(s: ProbeSession): string {
  return join(s.sb.home, ".config/opencode");
}

function readGlobal(s: ProbeSession, name: string): string {
  return readFileSync(join(globalDir(s), name), "utf8");
}

/** The plugin entries the host merged, serialized for matching. */
export async function mergedPlugins(ctx: EntryContext, s: ProbeSession): Promise<string[]> {
  const out = await host(ctx, s, ["debug", "config"]);
  const plugins = ResolvedPlugins.parse(JSON.parse(out.stdout)).plugin ?? [];
  return plugins.map((entry) => JSON.stringify(entry));
}

/** The toolu catalog skills the host discovered, sorted. */
export async function tooluSkills(ctx: EntryContext, s: ProbeSession): Promise<string[]> {
  return named((await skills(ctx, s)).rows, catalogIds().skills).toSorted();
}

function cliSession(ctx: EntryContext): ProbeSession {
  return entrySession(ctx, {
    files: GATED_FILES,
    config: () => ({ permission: { bash: "allow" } }),
    scripts: writeEnvScript,
  });
}

async function cliInstall(ctx: EntryContext): Promise<EntryResult> {
  using s = cliSession(ctx);
  const exit = await toolu(ctx, s, ["install"]);
  const written = readGlobal(s, "opencode.json");
  const discovered = await tooluSkills(ctx, s);
  const gate = await protectedWrite(ctx, s);
  const rerun = await toolu(ctx, s, ["install"]);
  const merged = await mergedPlugins(ctx, s);
  const observed = {
    exit,
    rerun,
    skills: discovered.length,
    catalogSkills: catalogIds().skills.size,
    entries: merged.filter((entry) => entry.includes(PACKAGE)).length,
    spec: merged.some((entry) => entry === JSON.stringify(npmSpec(ctx.tarball))),
    idempotent: readGlobal(s, "opencode.json") === written,
    noSelection:
      !s.exists(".opencode/toolu/plugins.json") &&
      !existsSync(join(globalDir(s), "toolu/plugins.json")),
    enforced: enforced(gate),
    ...gate,
  };
  const pass =
    exit === 0 &&
    rerun === 0 &&
    observed.skills === observed.catalogSkills &&
    observed.entries === 1 &&
    observed.spec &&
    observed.idempotent &&
    observed.noSelection &&
    observed.enforced;
  return { pass, observed };
}

function userConfig(probe: string): string {
  return [
    "// user settings, kept by toolu",
    "{",
    "  // an unrelated plugin",
    `  "plugin": [${JSON.stringify(probe)}],`,
    '  "share": "disabled",',
    "}",
    "",
  ].join("\n");
}

const startsWith = (names: readonly string[], prefix: string): boolean =>
  names.length > 0 && names.every((name) => name.startsWith(prefix));

async function cliLifecycle(ctx: EntryContext): Promise<EntryResult> {
  using s = cliSession(ctx);
  const probe = pathToFileURL(PROBE_PLUGIN).href;
  mkdirSync(globalDir(s), { recursive: true });
  writeFileSync(join(globalDir(s), "opencode.jsonc"), userConfig(probe));
  const exits = [await toolu(ctx, s, ["install", "jev"])];
  const named1 = await tooluSkills(ctx, s);
  const afterJev = readGlobal(s, "opencode.jsonc");
  exits.push(await toolu(ctx, s, ["install", "jev"]));
  const repeated = readGlobal(s, "opencode.jsonc") === afterJev;
  exits.push(await toolu(ctx, s, ["install", "context7"]));
  const both = await tooluSkills(ctx, s);
  exits.push(await toolu(ctx, s, ["remove", "--yes", "context7"]));
  const narrowed = await tooluSkills(ctx, s);
  const release = join(dirname(ctx.tarball), "toolu-opencode-next.tgz");
  cpSync(ctx.tarball, release);
  exits.push(await toolu(ctx, s, ["update"], npmSpec(release)));
  const updated = await mergedPlugins(ctx, s);
  const updatedSkills = await tooluSkills(ctx, s);
  exits.push(await toolu(ctx, s, ["remove", "--yes", "toolu"]));
  const removed = await mergedPlugins(ctx, s);
  const removedSkills = await tooluSkills(ctx, s);
  const finalText = readGlobal(s, "opencode.jsonc");
  const observed = {
    exits: exits.join(","),
    jevOnly: startsWith(named1, "jev-"),
    repeated,
    both: both.some((n) => n.startsWith("context7-")) && both.some((n) => n.startsWith("jev-")),
    narrowed: startsWith(narrowed, "jev-"),
    updated: updated.includes(JSON.stringify(npmSpec(release))) && updated.length === 2,
    updatedSkills: startsWith(updatedSkills, "jev-"),
    removed: removed.join(",") === JSON.stringify(probe),
    removedSkills: removedSkills.length,
    commentsKept: finalText.includes("// user settings") && finalText.includes("// an unrelated"),
  };
  const pass =
    observed.exits === "0,0,0,0,0,0" &&
    observed.jevOnly &&
    observed.repeated &&
    observed.both &&
    observed.narrowed &&
    observed.updated &&
    observed.updatedSkills &&
    observed.removed &&
    observed.removedSkills === 0 &&
    observed.commentsKept;
  return { pass, observed };
}

export const CLI_SCENARIOS: EntryScenario[] = [
  {
    id: "cli.install",
    claim:
      "`toolu install --host opencode` in a clean profile loads every plugin, enforces a gate, and a rerun changes nothing",
    run: cliInstall,
  },
  {
    id: "cli.lifecycle",
    claim:
      "Named install, enable, disable, update and remove edit a commented global opencode.jsonc the host merges, keeping the user's comments and plugin",
    run: cliLifecycle,
  },
];
