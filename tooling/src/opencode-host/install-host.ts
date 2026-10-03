/**
 * Shared host access for the live install and discovery scenarios (#345): an
 * isolated project loading the packed `@toolu/opencode` through the npm route,
 * and what the pinned host itself discovered (`debug skill`, the server's
 * `/agent` and `/command`, the native `skill` and `read` tools, the host log).
 */
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { RUN_TIMEOUT_MS, runHost, withServe } from "./host-run.ts";
import type { ScriptStep } from "./provider.ts";
import {
  ROOT,
  entrySession,
  npmSpec,
  type EntryContext,
  type EntryResult,
} from "./scenarios-entry.ts";
import { ContractError } from "./schema.ts";
import type { ProbeSession } from "./session.ts";

export type Scripts = Record<string, ScriptStep[]>;
export type Observed = EntryResult["observed"];

export const SELECTION = ".opencode/toolu/plugins.json";
const GENERATED = join(ROOT, "tools/toolu-opencode/generated");
const Ids = z.array(z.object({ id: z.string() }));
const CatalogSchema = z.object({
  plugins: z.array(z.looseObject({ skills: Ids, agents: Ids, commands: Ids })),
});
const SkillRows = z.array(
  z.looseObject({ name: z.string(), description: z.string().optional(), location: z.string() }),
);
type SkillRow = z.infer<typeof SkillRows>[number];
const Listed = z.array(z.looseObject({ name: z.string() }));
const ToolUse = z.looseObject({
  type: z.literal("tool_use"),
  part: z.looseObject({
    tool: z.string(),
    state: z.looseObject({ status: z.string(), output: z.unknown(), error: z.unknown() }),
  }),
});
const ChatMessages = z.looseObject({
  messages: z.array(z.looseObject({ role: z.string(), content: z.unknown() })),
});

/** Every catalog ID per kind, so toolu's surfaces can be told from the host's and the user's. */
export function catalogIds(): { skills: Set<string>; agents: Set<string>; commands: Set<string> } {
  const parsed = CatalogSchema.parse(
    JSON.parse(readFileSync(join(GENERATED, "opencode.toolu.json"), "utf8")),
  );
  const ids = (kind: "skills" | "agents" | "commands"): Set<string> =>
    new Set(parsed.plugins.flatMap((plugin) => plugin[kind].map((entry) => entry.id)));
  return { skills: ids("skills"), agents: ids("agents"), commands: ids("commands") };
}

export function selection(names: readonly string[]): string {
  return JSON.stringify({ version: 1, enabled: names });
}

export function skillFile(name: string, description: string): string {
  return `---\nname: ${name}\ndescription: ${description}\n---\n${description} body\n`;
}

/** A project loading the packed tarball through the npm route, plus `files` and `config`. */
export function install(
  ctx: EntryContext,
  files: Record<string, string>,
  scripts: Scripts = {},
  config: (root: string) => Record<string, unknown> = () => ({}),
): ProbeSession {
  return entrySession(ctx, {
    files,
    scripts,
    config: (root) => ({ plugin: [npmSpec(ctx.tarball)], ...config(root) }),
  });
}

/** Rewrite the project's `opencode.json`, keeping the session's provider entry. */
export function editConfig(s: ProbeSession, edit: (config: Record<string, unknown>) => void): void {
  const path = join(s.sb.project, "opencode.json");
  const config = z.record(z.string(), z.unknown()).parse(JSON.parse(readFileSync(path, "utf8")));
  edit(config);
  writeFileSync(path, `${JSON.stringify(config, null, 2)}\n`);
}

/** `opencode <args> --print-logs` in `cwd` (PWD follows it); stdout and the log text. */
export async function host(
  ctx: EntryContext,
  s: ProbeSession,
  args: string[],
  cwd = s.sb.project,
): Promise<{ stdout: string; log: string }> {
  const res = await run([ctx.bin, ...args, "--print-logs"], {
    cwd,
    env: { ...s.env, PWD: cwd },
    stdin: "",
    timeoutMs: RUN_TIMEOUT_MS,
  });
  if (res.timedOut || res.exitCode !== 0)
    throw new ContractError(
      `opencode ${args.join(" ")} failed (exit ${res.exitCode}): ${res.stderr.slice(-2000)}`,
    );
  return { stdout: res.stdout, log: res.stderr };
}

export async function skills(
  ctx: EntryContext,
  s: ProbeSession,
  cwd?: string,
): Promise<{ rows: SkillRow[]; log: string }> {
  const out = await host(ctx, s, ["debug", "skill"], cwd);
  return { rows: SkillRows.parse(JSON.parse(out.stdout)), log: out.log };
}

/** The host's own agent and command lists, from `opencode serve`. */
export async function services(
  ctx: EntryContext,
  s: ProbeSession,
): Promise<{ agents: Array<Record<string, unknown>>; commands: Array<Record<string, unknown>> }> {
  return withServe(ctx.bin, s, async (url) => {
    const query = `?directory=${encodeURIComponent(s.sb.project)}`;
    const list = async (path: string): Promise<Array<Record<string, unknown>>> =>
      Listed.parse(await (await fetch(`${url}${path}${query}`)).json());
    return { agents: await list("/agent"), commands: await list("/command") };
  });
}

export function named(list: ReadonlyArray<{ name: string }>, keep: Set<string>): string[] {
  return list.map((item) => item.name).filter((name) => keep.has(name));
}

export function countOf(rows: readonly SkillRow[], name: string): number {
  return rows.filter((row) => row.name === name).length;
}

/** `<…>/generated`, from a toolu skill the host located under `generated/skills/<id>/`. */
export function generatedRoot(rows: readonly SkillRow[], id: string): string {
  const row = rows.find((r) => r.name === id);
  if (row === undefined) throw new ContractError(`the host did not discover ${id}`);
  return dirname(dirname(dirname(row.location)));
}

export function lines(log: string, text: string): number {
  return log.split("\n").filter((line) => line.includes(text)).length;
}

/** Tool parts of `opencode run --format json`, with their outputs. */
export function toolResults(events: ReadonlyArray<Record<string, unknown>>) {
  return events.flatMap((event) => {
    const parsed = ToolUse.safeParse(event);
    if (!parsed.success) return [];
    const { tool, state } = parsed.data.part;
    return [{ tool, status: state.status, text: JSON.stringify([state.output, state.error]) }];
  });
}

/** Load a skill with the native tool and read a shared procedure it links. */
export async function skillAndRead(
  ctx: EntryContext,
  s: ProbeSession,
  scripts: Scripts,
  key: string,
  generated: string,
): Promise<{ skill: string; read: string; skillText: string; described: boolean }> {
  scripts[key] = [
    { tool: "skill", args: { name: "toolu-commit-e11d9d00" } },
    { tool: "read", args: { filePath: join(generated, "resources/toolu/workflows/commit.md") } },
  ];
  const results = toolResults((await runHost(ctx.bin, s, [`PROBE:${key}`])).events);
  const skill = results.find((r) => r.tool === "skill");
  const read = results.find((r) => r.tool === "read");
  // The pinned host lists available skills in the system prompt, not in the tool's description.
  const described = s.requests().some((request) => {
    const parsed = ChatMessages.safeParse(request.body);
    if (!parsed.success) return false;
    const system = parsed.data.messages.filter((m) => m.role === "system");
    return JSON.stringify(system).includes("toolu-commit-e11d9d00");
  });
  return {
    skill: skill?.status ?? "missing",
    read: read?.status ?? "missing",
    skillText: skill?.text ?? "",
    described,
  };
}

/** The project's discovery directories hold only `userSkills`: toolu wrote nothing there. */
export function noSurfaceFiles(s: ProbeSession, userSkills: readonly string[] = []): boolean {
  const skillsDir = join(s.sb.project, ".opencode/skills");
  const present = s.exists(".opencode/skills") ? readdirSync(skillsDir).toSorted() : [];
  const others = ["skill", "agents", "agent", "commands", "command"];
  return (
    present.join(",") === [...userSkills].toSorted().join(",") &&
    others.every((dir) => !s.exists(`.opencode/${dir}`))
  );
}

export function passes(observed: Observed, expected: Observed): boolean {
  return Object.entries(expected).every(([key, value]) => observed[key] === value);
}
