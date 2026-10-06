/**
 * Declarative sandboxes for toolu's lifecycle hooks (#263). Each case builds a
 * real git repository, real config and install-record files and real stub
 * executables, then runs one hook. The bash scripts produced
 * `fixtures/lifecycle-golden.json` from these cases before they were deleted;
 * `lifecycle-golden.test.ts` replays the Bun bundles against it.
 */
import { chmodSync, mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { PERMISSIONS_SENTINEL } from "@toolu/core/config";
import { z } from "zod";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch, type RunResult } from "@toolu/conformance/harness/spawn";

export const PLUGIN = resolve(import.meta.dir, "../../..");

export type LifecycleHook = "session-start" | "user-prompt-submit";

export type LifecycleCase = {
  readonly name: string;
  readonly hook: LifecycleHook;
  readonly stdin: string;
  readonly host?: "claude" | "codex" | undefined;
  /** Default true: the project is a git repo with one commit on `branch`. */
  readonly git?: boolean | undefined;
  readonly branch?: string | undefined;
  /** Run inside this child of the project (created, and made the repo when `git`). */
  readonly subdir?: string | undefined;
  /** Committed with the initial commit. */
  readonly files?: Readonly<Record<string, string>> | undefined;
  /** Written after the commit (config, gate file, `context.sh`); `.sh` files are executable. */
  readonly untracked?: Readonly<Record<string, string>> | undefined;
  /** User-scope toolu.config.json. */
  readonly userConfig?: object | undefined;
  /** Claude's installed_plugins.json; omitted means no file. */
  readonly registry?: object | undefined;
  /** Publish `<config root>/<name>/search.sh` for each name. */
  readonly wrappers?: readonly string[] | undefined;
  /** Replace CLAUDE_PLUGIN_ROOT with a synthetic plugin carrying these manifests. */
  readonly manifest?: object | undefined;
  readonly codexManifest?: object | undefined;
  /** `codex plugin list --json` output of a stub on PATH (Codex host only). */
  readonly codexList?: string | undefined;
  /** Put an `ast-grep` stub on PATH. */
  readonly astGrep?: boolean | undefined;
  /** Leave the one-time notices and the permission write unseen (default: seen). */
  readonly firstRun?: boolean | undefined;
  readonly env?: Readonly<Record<string, string>> | undefined;
};

export type Captured = { stdout: string; stderr: string; exitCode: number };

export type Golden = { base: string; cases: Record<string, Captured> };

const LifecycleCaseSchema = z.strictObject({
  name: z.string(),
  hook: z.enum(["session-start", "user-prompt-submit"]),
  stdin: z.string(),
  host: z.enum(["claude", "codex"]).optional(),
  git: z.boolean().optional(),
  branch: z.string().optional(),
  subdir: z.string().optional(),
  files: z.record(z.string(), z.string()).optional(),
  untracked: z.record(z.string(), z.string()).optional(),
  userConfig: z.record(z.string(), z.unknown()).optional(),
  registry: z.record(z.string(), z.unknown()).optional(),
  wrappers: z.array(z.string()).optional(),
  manifest: z.record(z.string(), z.unknown()).optional(),
  codexManifest: z.record(z.string(), z.unknown()).optional(),
  codexList: z.string().optional(),
  astGrep: z.boolean().optional(),
  firstRun: z.boolean().optional(),
  env: z.record(z.string(), z.string()).optional(),
});

/** Load either lifecycle hook's cases from the shared JSON contract. */
export function readLifecycleCases(hook: LifecycleHook): LifecycleCase[] {
  const path = resolve(import.meta.dir, "../../../../../fixtures/gates/lifecycle.json");
  return z
    .array(LifecycleCaseSchema)
    .parse(readCaseFile(path))
    .filter((item) => item.hook === hook);
}

function writeFile(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
  if (path.endsWith(".sh")) chmodSync(path, 0o755);
}

function stub(bin: string, name: string, body: string): void {
  writeFile(join(bin, name), `#!/bin/sh\n${body}\n`);
  chmodSync(join(bin, name), 0o755);
}

function configRoot(c: LifecycleCase, sb: Sandbox): string {
  return c.host === "codex" ? sb.codexHome : join(sb.home, ".claude");
}

function projectDirname(c: LifecycleCase): string {
  return c.host === "codex" ? ".codex" : ".claude";
}

/** The directory the hook runs in: the project, or its `subdir`. */
function workdir(c: LifecycleCase, sb: Sandbox): string {
  return c.subdir === undefined ? sb.project : join(sb.project, c.subdir);
}

function initGit(dir: string, branch: string): void {
  const git = (...args: string[]): void => {
    const res = Bun.spawnSync(["git", "-C", dir, ...args], { stderr: "pipe" });
    if (res.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr.toString()}`);
  };
  git("init", "-q", "-b", branch);
  git("config", "user.email", "t@t");
  git("config", "user.name", "t");
  git("config", "commit.gpgsign", "false");
  git("add", "-A");
  git("commit", "-q", "--allow-empty", "-m", "init");
}

function markSeen(c: LifecycleCase, sb: Sandbox, dir: string): void {
  const root = configRoot(c, sb);
  writeFile(join(root, "toolu", ".gate-preset-notice-v6"), "");
  writeFile(join(root, "toolu", ".delivery-flow-migration-v7"), "");
  writeFile(join(dir, projectDirname(c), "tmp", PERMISSIONS_SENTINEL), "");
}

function syntheticPlugin(c: LifecycleCase, sb: Sandbox): string | undefined {
  if (c.manifest === undefined) return undefined;
  const root = join(sb.root, "plug");
  writeFile(join(root, ".claude-plugin", "plugin.json"), `${JSON.stringify(c.manifest)}\n`);
  if (c.codexManifest !== undefined) {
    writeFile(join(root, ".codex-plugin", "plugin.json"), `${JSON.stringify(c.codexManifest)}\n`);
  }
  return root;
}

/** The only tools on a case's PATH besides its stubs: what the bundles spawn. */
const LINKED = ["git", "bash"];

function binDir(c: LifecycleCase, sb: Sandbox): string {
  const bin = join(sb.root, "bin");
  mkdirSync(bin, { recursive: true });
  for (const tool of LINKED) {
    const found = Bun.which(tool);
    if (found !== null) symlinkSync(found, join(bin, tool));
  }
  if (c.astGrep === true) stub(bin, "ast-grep", "exit 0");
  if (c.codexList !== undefined) {
    stub(bin, "codex", `printf '%s\\n' '${c.codexList}'`);
  }
  return bin;
}

export type Prepared = { sb: Sandbox; cwd: string; env: EnvPatch };

/** Build `c`'s sandbox. */
export function prepare(c: LifecycleCase): Prepared {
  const sb = createSandbox();
  const cwd = workdir(c, sb);
  mkdirSync(cwd, { recursive: true });
  for (const [rel, body] of Object.entries(c.files ?? {})) writeFile(join(cwd, rel), body);
  if (c.git !== false) initGit(cwd, c.branch ?? "main");
  for (const [rel, body] of Object.entries(c.untracked ?? {})) writeFile(join(cwd, rel), body);
  if (c.firstRun !== true) markSeen(c, sb, cwd);
  const root = configRoot(c, sb);
  if (c.userConfig !== undefined) {
    writeFile(join(root, "toolu.config.json"), `${JSON.stringify(c.userConfig)}\n`);
  }
  for (const name of c.wrappers ?? []) writeFile(join(root, name, "search.sh"), "exit 0\n");
  const registry = join(sb.root, "installed_plugins.json");
  if (c.registry !== undefined) writeFile(registry, JSON.stringify(c.registry));
  const plugin = syntheticPlugin(c, sb) ?? PLUGIN;
  const bin = binDir(c, sb);
  const env: EnvPatch = {
    PATH: bin,
    HOME: sb.home,
    LANG: "C",
    LC_ALL: "C",
    EXA_API_KEY: undefined,
    CLAUDE_PLUGIN_ROOT: plugin,
    CLAUDE_PLUGINS_REGISTRY: registry,
    ...(c.host === "codex"
      ? { TOOLU_HOST_OVERRIDE: "codex", CODEX_HOME: sb.codexHome, PLUGIN_ROOT: plugin }
      : { CLAUDE_PROJECT_DIR: cwd }),
    ...c.env,
  };
  return { sb, cwd, env };
}

/**
 * Replace per-run paths so outputs compare across sandboxes and machines.
 * `sb.root` is a fresh `mkdtemp` directory, so it can only appear in output
 * that names the sandbox itself, which is exactly what should be masked.
 */
export function mask(text: string, sb: Sandbox): string {
  return text.replaceAll(sb.root, "<SANDBOX>").replaceAll(PLUGIN, "<PLUGIN>");
}

/** Run `argv` in `c`'s sandbox and return the masked result. */
export async function runCase(
  c: LifecycleCase,
  argv: (hook: LifecycleHook) => string[],
): Promise<Captured> {
  const { sb, cwd, env } = prepare(c);
  try {
    const res: RunResult = await run(argv(c.hook), { cwd, env, stdin: c.stdin });
    return { stdout: mask(res.stdout, sb), stderr: mask(res.stderr, sb), exitCode: res.exitCode };
  } finally {
    sb[Symbol.dispose]();
  }
}

const HooksFileSchema = z.object({
  hooks: z.record(
    z.string(),
    z.array(z.object({ hooks: z.array(z.object({ command: z.string() })) })),
  ),
});

/** How many of the plugin's `event` hooks launch `hooks/dist/<entry>.js`. */
export async function launchCount(event: string, entry: string): Promise<number> {
  const file = HooksFileSchema.parse(await Bun.file(join(PLUGIN, "hooks", "hooks.json")).json());
  const bundle = `/hooks/dist/${entry}.js"`;
  return (file.hooks[event] ?? [])
    .flatMap((group) => group.hooks)
    .filter((hook) => hook.command.includes(bundle)).length;
}

/** The committed bundle for `hook`, run by the Bun running the tests, or its selected Rust command. */
export function bundleArgv(hook: LifecycleHook): string[] {
  return entryArgv("toolu", hook, PLUGIN);
}
