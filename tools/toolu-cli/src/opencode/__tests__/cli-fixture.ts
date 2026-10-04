/**
 * An isolated OpenCode profile for subprocess tests: a fresh HOME and XDG
 * config root plus a git project, and the CLI source run against them.
 */
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { z } from "zod";

const CLI = resolve(import.meta.dir, "../../cli.ts");
export const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const manifest = z
  .looseObject({ version: z.string() })
  .parse(JSON.parse(readFileSync(resolve(import.meta.dir, "../../../package.json"), "utf8")));
export const TARGET = `@toolu/opencode@${manifest.version}`;

const selectionSchema = z.object({ version: z.literal(1), enabled: z.array(z.string()) });
const rowsSchema = z.array(
  z.object({
    name: z.string(),
    installed: z.boolean(),
    version: z.string().optional(),
    enabled: z.boolean(),
  }),
);

export interface Profile {
  readonly home: string;
  readonly project: string;
  readonly env: Record<string, string>;
  /** `<home>/.config/opencode/<rel>`. */
  global(rel: string): string;
  /** `<project>/<rel>`. */
  local(rel: string): string;
  write(path: string, text: string): void;
  read(path: string): string;
  exists(path: string): boolean;
  [Symbol.dispose](): void;
}

export function profile(): Profile {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "toolu-cli-opencode-")));
  const home = join(root, "home");
  const project = join(root, "project");
  mkdirSync(home, { recursive: true });
  mkdirSync(project, { recursive: true });
  spawnSync("git", ["init", "-q", project]);
  const env = {
    PATH: process.env.PATH ?? "/usr/bin:/bin",
    HOME: home,
    XDG_CONFIG_HOME: join(home, ".config"),
    TERM: "dumb",
  };
  return {
    home,
    project,
    env,
    global: (rel) => join(home, ".config/opencode", rel),
    local: (rel) => join(project, rel),
    write: (path, text) => {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, text);
    },
    read: (path) => readFileSync(path, "utf8"),
    exists: (path) => existsSync(path),
    [Symbol.dispose]: () => rmSync(root, { recursive: true, force: true }),
  };
}

export interface CliRun {
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;
}

/** `toolu <args> --host opencode` in the project, with `extra` env on top of the profile's. */
export function toolu(
  p: Profile,
  args: readonly string[],
  extra: Record<string, string> = {},
): CliRun {
  const res = spawnSync(process.execPath, [CLI, ...args, "--host", "opencode"], {
    cwd: p.project,
    env: { ...p.env, ...extra },
    encoding: "utf8",
  });
  return { code: res.status ?? 1, stdout: res.stdout, stderr: res.stderr };
}

export function selection(enabled: readonly string[]): string {
  return `${JSON.stringify({ version: 1, enabled }, null, 2)}\n`;
}

export function enabledOf(text: string): readonly string[] {
  return selectionSchema.parse(JSON.parse(text)).enabled;
}

/** `list --json` rows keyed by plugin name. */
export function listed(p: Profile): Map<string, z.infer<typeof rowsSchema>[number]> {
  const run = toolu(p, ["list", "--json"]);
  if (run.code !== 0) throw new Error(`list failed: ${run.stderr}`);
  return new Map(rowsSchema.parse(JSON.parse(run.stdout)).map((row) => [row.name, row]));
}

/** The names `list --json` reports enabled, sorted. */
export function enabledNames(p: Profile): readonly string[] {
  return [...listed(p).values()]
    .filter((row) => row.enabled)
    .map((row) => row.name)
    .toSorted();
}
