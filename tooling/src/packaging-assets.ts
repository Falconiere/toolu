/**
 * The asset half of validate-plugin-packaging: skill frontmatter, Codex agent
 * TOML and hook manifests, plus the Repo reader and failure type both halves share.
 */
import { existsSync, lstatSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { get, isNullish, list } from "./json-path.ts";

export const EXPECTED = { plugins: 16, skills: 18, agents: 5, hooks: 14 };
const REQUIRED_SKILLS = [
  "plugins/toolu/skills/commit/SKILL.md",
  "plugins/toolu/skills/review-and-commit/SKILL.md",
  "plugins/toolu/skills/setup/SKILL.md",
  "plugins/statusline/skills/status/SKILL.md",
  "plugins/pr-babysit/skills/babysit/SKILL.md",
];
const LAUNCHER = /hooks\/dist\/|\bbun\b/;

export class PackagingError extends Error {}

export function fail(message: string): never {
  throw new PackagingError(message);
}

export class Repo {
  constructor(readonly root: string) {}

  path(rel: string): string {
    return join(this.root, rel);
  }

  json(rel: string): unknown {
    if (!existsSync(this.path(rel))) fail(`missing ${rel}`);
    try {
      return JSON.parse(readFileSync(this.path(rel), "utf8"));
    } catch {
      return fail(`invalid JSON: ${rel}`);
    }
  }

  /** `dir/*` entries that are directories, sorted like a shell glob. */
  dirs(rel: string): string[] {
    if (!existsSync(this.path(rel))) return [];
    return readdirSync(this.path(rel), { withFileTypes: true })
      .filter(
        (e) => !e.name.startsWith(".") && statSync(join(this.path(rel), e.name)).isDirectory(),
      )
      .map((e) => `${rel}/${e.name}`)
      .toSorted();
  }
}

function checkSkill(repo: Repo, file: string): void {
  const lines = readFileSync(repo.path(file), "utf8").split("\n");
  if (lines[0] !== "---") fail(`${file} is missing opening frontmatter`);
  const name = /^name:\s*(.*)$/.exec(lines[1] ?? "")?.[1] ?? "";
  const description = /^description:\s*(.*)$/.exec(lines[2] ?? "")?.[1] ?? "";
  if (name === "") fail(`${file} is missing a frontmatter name`);
  if (description === "") fail(`${file} is missing a frontmatter description`);
  if (!/^[a-z0-9-]+$/.test(name)) fail(`${file} has an invalid skill name`);
  if (name !== basename(dirname(file))) fail(`${file} name differs from its directory`);
  if (!lines.slice(1).includes("---")) fail(`${file} is missing closing frontmatter`);
}

export function checkSkills(repo: Repo): number {
  const files = repo
    .dirs("plugins")
    .flatMap((plugin) => repo.dirs(`${plugin}/skills`))
    .map((dir) => `${dir}/SKILL.md`)
    .filter((file) => existsSync(repo.path(file)));
  for (const file of files) checkSkill(repo, file);
  if (files.length !== EXPECTED.skills) {
    fail(`expected ${String(EXPECTED.skills)} discoverable skills, found ${String(files.length)}`);
  }
  for (const skill of REQUIRED_SKILLS) {
    if (!existsSync(repo.path(skill))) fail(`missing Codex command-equivalent skill: ${skill}`);
  }
  return files.length;
}

function checkAgent(repo: Repo, file: string): void {
  let doc: unknown;
  try {
    doc = Bun.TOML.parse(readFileSync(repo.path(file), "utf8"));
  } catch {
    fail(`invalid agent TOML: ${file}`);
  }
  const text = (key: string): boolean => {
    const value = get(doc, key);
    return typeof value === "string" && value.trim().length > 0;
  };
  const fields = [
    "name",
    "description",
    "model",
    "model_reasoning_effort",
    "sandbox_mode",
    "developer_instructions",
  ];
  const effort = get(doc, "model_reasoning_effort");
  const sandbox = get(doc, "sandbox_mode");
  if (
    !fields.every(text) ||
    !["low", "medium", "high", "xhigh", "max", "ultra"].some((e) => e === effort) ||
    !["read-only", "workspace-write"].some((s) => s === sandbox)
  ) {
    fail(`invalid agent TOML: ${file}`);
  }
  if (get(doc, "name") !== basename(file, ".toml")) fail(`${file} name differs from its filename`);
}

export function checkAgents(repo: Repo): number {
  const dir = "plugins/toolu/assets/agents";
  const files = existsSync(repo.path(dir))
    ? readdirSync(repo.path(dir))
        .filter((f) => f.endsWith(".toml"))
        .toSorted()
        .map((f) => `${dir}/${f}`)
    : [];
  for (const file of files) checkAgent(repo, file);
  if (files.length !== EXPECTED.agents) {
    fail(`expected ${String(EXPECTED.agents)} Codex agent profiles, found ${String(files.length)}`);
  }
  return files.length;
}

function checkHookFile(repo: Repo, file: string): void {
  const doc = repo.json(file);
  const hooks = get(doc, "hooks");
  const groups =
    typeof hooks === "object" && hooks !== null && !Array.isArray(hooks)
      ? Object.values(hooks)
      : null;
  const valid =
    groups?.every(
      (value) =>
        Array.isArray(value) &&
        value.every((group) => {
          const inner = get(group, "hooks");
          return (
            Array.isArray(inner) &&
            inner.every(
              (h) => get(h, "type") === "command" && typeof get(h, "command") === "string",
            )
          );
        }),
    ) ?? false;
  if (groups === null || !valid) fail(`invalid hook schema: ${file}`);
  const pluginRoot = file.slice(0, -"/hooks/hooks.json".length);
  const commands = groups.flatMap(list).flatMap((group) => list(get(group, "hooks")));
  for (const hook of commands) {
    const raw = get(hook, "command");
    if (typeof raw !== "string" || !isNullish(get(hook, "commandWindows")) || LAUNCHER.test(raw))
      continue;
    // A single quoted executable path is valid for plugin caches with spaces.
    const command =
      raw.length > 1 && raw.startsWith('"') && raw.endsWith('"') ? raw.slice(1, -1) : raw;
    const prefix = ["${CLAUDE_PLUGIN_ROOT}/", "${PLUGIN_ROOT}/"].find((p) => command.startsWith(p));
    if (prefix === undefined) fail(`${file} has a non-plugin-relative hook command: ${command}`);
    const hookPath = command.slice(prefix.length);
    const abs = repo.path(`${pluginRoot}/${hookPath}`);
    if (!existsSync(abs) || !lstatSync(abs).isFile() || (statSync(abs).mode & 0o111) === 0) {
      fail(`${file} references a missing or non-executable hook: ${hookPath}`);
    }
  }
}

export function checkHooks(repo: Repo): number {
  const files = repo
    .dirs("plugins")
    .map((plugin) => `${plugin}/hooks/hooks.json`)
    .filter((file) => existsSync(repo.path(file)));
  for (const file of files) checkHookFile(repo, file);
  if (files.length !== EXPECTED.hooks) {
    fail(`expected ${String(EXPECTED.hooks)} hook manifests, found ${String(files.length)}`);
  }
  return files.length;
}
