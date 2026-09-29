/** Install toolu's Codex custom-agent profiles without clobbering user files.
 *
 * Usage: bun setup.ts preview | install [--force] | remove --yes [--force] */

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  utimesSync,
  chmodSync,
} from "node:fs";
import { resolve } from "node:path";

const MANAGED_MARKER = "# Managed by toolu. Install and update with $toolu:setup.";
const USAGE = "Usage: setup.ts preview | install [--force] | remove --yes [--force]\n";

type Profile = { name: string; model: string; effort: string; sandbox: string };

export const PROFILES: Profile[] = [
  { name: "quick-task", model: "gpt-5.6-luna", effort: "medium", sandbox: "read-only" },
  { name: "deep-explore", model: "gpt-5.6-terra", effort: "medium", sandbox: "read-only" },
  { name: "research-agent", model: "gpt-5.6-terra", effort: "medium", sandbox: "read-only" },
  { name: "implementer", model: "gpt-5.6-terra", effort: "medium", sandbox: "workspace-write" },
  { name: "architect", model: "gpt-5.6-sol", effort: "high", sandbox: "read-only" },
];

type Action = "install" | "unchanged" | "update" | "conflict" | "absent" | "remove";

class SetupError extends Error {}

/** Lines of a file as grep and awk see them, or null when unreadable. */
function fileLines(path: string): string[] | null {
  let text: string;
  try {
    text = readFileSync(path, "latin1");
  } catch {
    return null;
  }
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

const isFile = (path: string) => existsSync(path) && statSync(path).isFile();

const ASSIGNMENT =
  /^(name|description|model|model_reasoning_effort|sandbox_mode) = "[^"\\]*(\\.[^"\\]*)*"$/s;

/** Only comments, blanks, the five string assignments and one developer_instructions block. */
function wellFormed(lines: string[]): boolean {
  let block = false;
  let assignments = 0;
  let starts = 0;
  let closes = 0;
  for (const line of lines) {
    if (line.startsWith("#") || /^[ \t\n\v\f\r]*$/.test(line)) continue;
    if (block) {
      if (line === '"""') {
        block = false;
        closes++;
      }
      continue;
    }
    if (line === 'developer_instructions = """') {
      block = true;
      starts++;
    } else if (ASSIGNMENT.test(line)) {
      assignments++;
    } else {
      return false;
    }
  }
  return !block && assignments === 5 && starts === 1 && closes === 1;
}

function validTemplate(file: string, profile: Profile): boolean {
  if (!isFile(file)) return false;
  const lines = fileLines(file);
  if (lines === null) return false;
  const once = (want: string) => lines.filter((line) => line === want).length === 1;
  return (
    once(MANAGED_MARKER) &&
    once(`name = "${profile.name}"`) &&
    lines.filter((line) => /^description = "[^"].*"$/s.test(line)).length === 1 &&
    once(`model = "${profile.model}"`) &&
    once(`model_reasoning_effort = "${profile.effort}"`) &&
    once(`sandbox_mode = "${profile.sandbox}"`) &&
    once('developer_instructions = """') &&
    once('"""') &&
    wellFormed(lines)
  );
}

const managed = (path: string) => fileLines(path)?.includes(MANAGED_MARKER) ?? false;

function sameBytes(a: string, b: string): boolean {
  try {
    return readFileSync(a).equals(readFileSync(b));
  } catch {
    return false;
  }
}

function classifyInstall(source: string, target: string): Action {
  if (!existsSync(target)) return "install";
  if (sameBytes(source, target)) return "unchanged";
  return managed(target) ? "update" : "conflict";
}

function classifyRemove(target: string): Action {
  if (!existsSync(target)) return "absent";
  return managed(target) ? "remove" : "conflict";
}

function backupStamp(): string {
  const stamp =
    process.env.TOOLU_TIMESTAMP ||
    new Date()
      .toISOString()
      .replace(/[-:]/g, "")
      .replace(/\.\d+Z$/, "Z");
  if (!/^[0-9TZ]+$/.test(stamp)) throw new SetupError(`invalid backup timestamp: ${stamp}`);
  return stamp;
}

/** `cp -p`: contents, mode and times. */
function copyPreserving(from: string, to: string): void {
  copyFileSync(from, to);
  const st = statSync(from);
  chmodSync(to, st.mode & 0o7777);
  utimesSync(to, st.atime, st.mtime);
}

/** Copy to a pid-named temp file beside the target, then rename over it. */
function writeAtomically(source: string, agentsDir: string, name: string): void {
  const tmp = `${agentsDir}/.${name}.toml.toolu.${process.pid}`;
  copyFileSync(source, tmp);
  renameSync(tmp, `${agentsDir}/${name}.toml`);
}

type Plan = { command: string; force: boolean; confirmed: boolean };

function parseArgs(argv: string[]): Plan | null {
  const [command = "", ...flags] = argv;
  let force = false;
  let confirmed = false;
  for (const flag of flags) {
    if (flag === "--force") force = true;
    else if (flag === "--yes") confirmed = true;
    else return null;
  }
  if (!["preview", "install", "remove"].includes(command)) return null;
  return { command, force, confirmed };
}

function apply(
  command: string,
  actions: Action[],
  dirs: { templates: string; agents: string; backup: string },
): void {
  let [installed, updated, unchanged, removed, absent] = [0, 0, 0, 0, 0];
  PROFILES.forEach(({ name }, index) => {
    const action = actions[index];
    const source = `${dirs.templates}/${name}.toml`;
    const target = `${dirs.agents}/${name}.toml`;
    if (command === "install") {
      if (action === "unchanged") unchanged++;
      if (action === "update" || action === "conflict") {
        copyPreserving(target, `${dirs.backup}/${name}.toml`);
        writeAtomically(source, dirs.agents, name);
        updated++;
      }
      if (action === "install") {
        writeAtomically(source, dirs.agents, name);
        installed++;
      }
    } else if (action === "absent") {
      absent++;
    } else if (action === "remove" || action === "conflict") {
      renameSync(target, `${dirs.backup}/${name}.toml`);
      removed++;
    }
  });
  if (command === "install") {
    console.log(`INSTALLED ${installed} UPDATED ${updated} UNCHANGED ${unchanged}`);
  } else {
    console.log(`REMOVED ${removed} ABSENT ${absent}`);
  }
}

function run(argv: string[]): number {
  const plan = parseArgs(argv);
  if (plan === null) {
    process.stderr.write(USAGE);
    return 2;
  }
  const templates =
    process.env.TOOLU_AGENT_TEMPLATE_DIR || `${resolve(import.meta.dir, "../../..")}/assets/agents`;
  const codexRoot =
    process.env.CODEX_HOME || (process.env.HOME ? `${process.env.HOME}/.codex` : "");
  if (codexRoot === "") throw new SetupError("CODEX_HOME and HOME are both unset");
  const agents = `${codexRoot}/agents`;

  for (const profile of PROFILES) {
    const file = `${templates}/${profile.name}.toml`;
    if (!validTemplate(file, profile)) throw new SetupError(`invalid agent template: ${file}`);
  }

  console.log(`TARGET ${agents}`);
  const actions = PROFILES.map(({ name }) => {
    const target = `${agents}/${name}.toml`;
    const action =
      plan.command === "remove"
        ? classifyRemove(target)
        : classifyInstall(`${templates}/${name}.toml`, target);
    console.log(`PLAN ${name} ${action}`);
    return action;
  });
  const conflicts = actions.filter((action) => action === "conflict").length;

  if (plan.command === "preview") {
    if (conflicts > 0)
      console.log("NOTICE use install --force only after confirming each conflict");
    return 0;
  }
  if (plan.command === "remove" && !plan.confirmed) {
    process.stderr.write("REFUSED removal requires --yes after explicit user confirmation\n");
    return 2;
  }
  if (conflicts > 0 && !plan.force) {
    process.stderr.write(
      "REFUSED unmanaged profile conflict; preview it and confirm install/remove --force\n",
    );
    return 2;
  }

  const backedUp = PROFILES.filter((_, index) =>
    ["update", "remove", "conflict"].includes(actions[index] ?? ""),
  );
  let backup = "";
  if (backedUp.length > 0) {
    backup = `${agents}/.toolu-backups/${backupStamp()}`;
    for (const { name } of backedUp) {
      const file = `${backup}/${name}.toml`;
      if (existsSync(file)) throw new SetupError(`backup already exists: ${file}`);
    }
  }

  mkdirSync(agents, { recursive: true });
  if (backup !== "") mkdirSync(backup, { recursive: true });
  apply(plan.command, actions, { templates, agents, backup });

  if (backup !== "") console.log(`BACKUP ${backup}`);
  console.log("Restart Codex to reload custom agent profiles.");
  return 0;
}

function main(argv: string[]): number {
  try {
    return run(argv);
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`toolu setup: ${message}\n`);
    return 1;
  }
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
