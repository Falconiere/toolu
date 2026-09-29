/**
 * Workspace smoke (`bun run test:workspace`): the required packages exist,
 * their exports resolve through Bun module resolution from the repo root, the
 * CLI reports the workspace version, and Bun holds the 1.4.x pin.
 * `CHECK_WORKSPACE_ROOT` points it at another checkout.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { z } from "zod";
import { envOr } from "./env.ts";

const PACKAGES = [
  "packages/toolu-core",
  "tools/toolu-opencode",
  "tools/toolu-conformance",
  "tools/toolu-cli",
  "tooling",
];
const WORKSPACES = [
  "packages/toolu-core",
  "tools/toolu-opencode",
  "tools/toolu-conformance",
  "tools/toolu-cli",
];

const RootManifest = z.looseObject({ workspaces: z.array(z.string()) });
const Versioned = z.looseObject({ version: z.string() });

class WorkspaceError extends Error {}

/** A manifest parsed against `schema`; unreadable or malformed ones name the file. */
function readManifest<T>(root: string, rel: string, schema: z.ZodType<T>): T {
  let doc: unknown;
  try {
    doc = JSON.parse(readFileSync(resolve(root, rel), "utf8"));
  } catch {
    throw new WorkspaceError(`${rel} is not readable JSON`);
  }
  const parsed = schema.safeParse(doc);
  if (!parsed.success)
    throw new WorkspaceError(`${rel} has an unexpected shape: ${parsed.error.message}`);
  return parsed.data;
}

/** Each export resolves through the workspace and returns what it promises. */
async function exportSmokes(): Promise<void> {
  const { parseDecision } = await import("@toolu/core/decision");
  if (parseDecision({ kind: "allow" }).kind !== "allow") throw new WorkspaceError("decision");
  const { detectHost } = await import("@toolu/core/host");
  if (detectHost({ env: { PLUGIN_ROOT: "/p" } }) !== "codex") throw new WorkspaceError("host");
  const { classifyStub } = await import("@toolu/opencode/plugin-stub");
  if (classifyStub("shell-out") !== "shell-out") throw new WorkspaceError("classify");
  const plugin = (await import("@toolu/opencode")).default;
  if (plugin.id !== "toolu") throw new WorkspaceError("default entry");
  const { runProtectedFilesConformance } = await import("@toolu/conformance/run-stub");
  const conformance = await runProtectedFilesConformance();
  if (!conformance.pass) throw new WorkspaceError(conformance.message);
}

async function check(root: string): Promise<void> {
  const version = Bun.version;
  if (!version.startsWith("1.4.")) {
    throw new WorkspaceError(`Bun ${version} not in supported 1.4.x range (docs/portable-core.md)`);
  }
  for (const pkg of PACKAGES) {
    if (!existsSync(resolve(root, pkg, "package.json")))
      throw new WorkspaceError(`missing ${pkg}/package.json`);
  }
  const manifest = readManifest(root, "package.json", RootManifest);
  if (!WORKSPACES.every((ws) => manifest.workspaces.includes(ws))) {
    throw new WorkspaceError("package.json workspaces must include tooling, packages/*, tools/*");
  }
  await exportSmokes();
  const res = spawnSync(process.execPath, ["run", "tools/toolu-cli/src/cli.ts", "--version"], {
    cwd: root,
    encoding: "utf8",
  });
  if (res.error !== undefined || res.status !== 0) {
    throw new WorkspaceError(
      `toolu-cli --version failed: ${res.error?.message ?? res.stderr.trim()}`,
    );
  }
  const cli = res.stdout.trim();
  const expected = readManifest(root, "tools/toolu-cli/package.json", Versioned);
  if (cli !== expected.version) throw new WorkspaceError(`toolu-cli --version reported '${cli}'`);
}

async function main(): Promise<number> {
  const root = envOr("CHECK_WORKSPACE_ROOT", resolve(import.meta.dir, "../.."));
  try {
    await check(root);
  } catch (err: unknown) {
    if (!(err instanceof WorkspaceError)) throw err;
    console.error(`check-workspace: ${err.message}`);
    return 1;
  }
  process.stdout.write("check-workspace: ok\n");
  return 0;
}

if (import.meta.main) process.exitCode = await main();
