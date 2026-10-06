/** Shared PreToolUse corpus loaded from declarative, host-neutral JSON. */
import { cpSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { z } from "zod";
import { toStdin } from "./fixtures.ts";
import {
  applyCaseSetup,
  HostSetupSchema,
  materializeToolFixture,
  readCaseFile,
} from "./json-cases.ts";
import { installPlugins, registerPlugin, TOOLU_PLUGIN, type PretoolHost } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";
import type { EnvPatch } from "./spawn.ts";

const OUTCOME = z.enum(["deny", "ask", "advisory", "silent", "exit2"]);
const CASE = z.strictObject({
  name: z.string().min(1),
  fixture: z.unknown().optional(),
  stdin: z.string().optional(),
  config: z.record(z.string(), z.json()).optional(),
  settings: z.record(z.string().regex(/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/), z.string()).optional(),
  setupByHost: HostSetupSchema,
  expect: z.strictObject({ claude: OUTCOME, codex: OUTCOME.optional() }),
});

export type Outcome = z.infer<typeof OUTCOME>;
export type PretoolCase = z.infer<typeof CASE>;

const CORPUS_PATH = resolve(import.meta.dir, "../../../../fixtures/gates/pretool-corpus.json");
export const PRETOOL_CORPUS: readonly PretoolCase[] = readCaseFile(CORPUS_PATH).map((item) =>
  CASE.parse(item),
);

const TOOL_REGISTRY_SPECS = ["toolu@toolu", "ast-grep@toolu", "fixture@toolu"];

/** Settings: a copy of the shipped settings with extra lines appended. */
function settingsDir(sb: Sandbox, extra: Record<string, string>): string {
  const dir = join(sb.root, "settings");
  cpSync(join(TOOLU_PLUGIN, "settings"), dir, { recursive: true });
  for (const [file, lines] of Object.entries(extra)) {
    const path = join(dir, file);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, lines);
  }
  return dir;
}

/** Prepare the real sandbox, then replay this host's bounded setup actions. */
export async function prepare(sb: Sandbox, host: PretoolHost, c: PretoolCase): Promise<EnvPatch> {
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  installPlugins(sb, ...TOOL_REGISTRY_SPECS);
  await registerPlugin(sb, host, "ast-grep");
  if (c.config !== undefined) sb.writeConfig(host, "project", c.config);
  applyCaseSetup(sb, c.setupByHost[host], host);
  return c.settings === undefined ? {} : { TOOLU_SETTINGS_DIR: settingsDir(sb, c.settings) };
}

/** Encode a committed fixture through the host's actual hook envelope. */
export function pretoolStdin(sb: Sandbox, host: PretoolHost, c: PretoolCase): string {
  if (c.stdin !== undefined) return c.stdin;
  if (c.fixture === undefined) throw new Error(`${c.name}: neither a fixture nor stdin`);
  return JSON.stringify(
    toStdin(host, materializeToolFixture(sb, c.fixture, host), { cwd: sb.project }),
  );
}
