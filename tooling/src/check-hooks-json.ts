/**
 * hooks.json launcher gate (#250). Every hook that starts a Bun bundle must carry
 * exactly the `command` / `commandWindows` pair `@toolu/core/launcher` generates,
 * so no plugin can hand-edit the fail-closed contract away.
 *
 * A hook counts as a launcher hook when its command names `hooks/dist/`, runs
 * `bun`, or carries `commandWindows`. Legacy script commands pass untouched until
 * their plugin is ported (epic #247).
 *
 * Usage: bun run tooling/src/check-hooks-json.ts [--root <dir>]
 *        bun run tooling/src/check-hooks-json.ts --print <plugin> <event> <entry>
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { launcherHook, type LauncherHook } from "@toolu/core/launcher";
import { z } from "zod";

export interface HooksJsonProblem {
  /** Repository-relative hooks.json path. */
  readonly file: string;
  /** `<Event>[i].hooks[j]`. */
  readonly where: string;
  readonly problem: string;
  readonly expected?: string;
}

const HookSchema = z
  .object({
    type: z.string(),
    command: z.string().optional(),
    commandWindows: z.string().optional(),
  })
  .passthrough();
const HooksFileSchema = z
  .object({
    hooks: z.record(z.string(), z.array(z.object({ hooks: z.array(HookSchema) }).passthrough())),
  })
  .passthrough();

type Hook = z.infer<typeof HookSchema>;

const DIST = /hooks[/\\]dist[/\\]([^"'\s/\\]+)\.js/;

function isLauncherHook(hook: Hook): boolean {
  const command = hook.command ?? "";
  return (
    hook.commandWindows !== undefined || command.includes("hooks/dist/") || /\bbun\b/.test(command)
  );
}

function entryOf(hook: Hook): string | undefined {
  return (DIST.exec(hook.command ?? "") ?? DIST.exec(hook.commandWindows ?? ""))?.[1];
}

function expectedHook(plugin: string, event: string, entry: string): LauncherHook | string {
  try {
    return launcherHook({ plugin, event, entry });
  } catch (error) {
    if (error instanceof Error) return error.message;
    throw error;
  }
}

interface HookSite {
  readonly root: string;
  readonly plugin: string;
  readonly event: string;
  readonly file: string;
  readonly where: string;
}

function checkHook(site: HookSite, hook: Hook): HooksJsonProblem[] {
  const { file, where } = site;
  const problem = (text: string, expected?: string): HooksJsonProblem =>
    expected === undefined
      ? { file, where, problem: text }
      : { file, where, problem: text, expected };
  const entry = entryOf(hook);
  if (entry === undefined) return [problem("launcher hook names no hooks/dist/<entry>.js bundle")];
  const expected = expectedHook(site.plugin, site.event, entry);
  if (typeof expected === "string") return [problem(expected)];
  const problems: HooksJsonProblem[] = [];
  if (hook.type !== "command")
    problems.push(problem(`type must be "command", got ${JSON.stringify(hook.type)}`));
  if (hook.command !== expected.command) {
    problems.push(problem("command differs from the generated launcher", expected.command));
  }
  if (hook.commandWindows !== expected.commandWindows) {
    problems.push(
      problem("commandWindows differs from the generated launcher", expected.commandWindows),
    );
  }
  const bundle = `plugins/${site.plugin}/hooks/dist/${entry}.js`;
  if (!existsSync(join(site.root, bundle)))
    problems.push(problem(`bundle ${bundle} is not committed`));
  return problems;
}

function checkFile(root: string, plugin: string): HooksJsonProblem[] {
  const file = `plugins/${plugin}/hooks/hooks.json`;
  let parsed: z.infer<typeof HooksFileSchema>;
  try {
    parsed = HooksFileSchema.parse(JSON.parse(readFileSync(join(root, file), "utf8")));
  } catch (error) {
    if (!(error instanceof Error)) throw error;
    return [{ file, where: "", problem: `invalid hooks.json: ${error.message.split("\n")[0]}` }];
  }
  return Object.entries(parsed.hooks).flatMap(([event, groups]) =>
    groups.flatMap((group, i) =>
      group.hooks.flatMap((hook, j) =>
        isLauncherHook(hook)
          ? checkHook({ root, plugin, event, file, where: `${event}[${i}].hooks[${j}]` }, hook)
          : [],
      ),
    ),
  );
}

/** Every launcher problem in `<root>/plugins/*\/hooks/hooks.json`, in plugin order. */
export function checkHooksJson(root: string): HooksJsonProblem[] {
  const plugins = join(root, "plugins");
  return readdirSync(plugins, { withFileTypes: true })
    .filter(
      (item) => item.isDirectory() && existsSync(join(plugins, item.name, "hooks/hooks.json")),
    )
    .map((item) => item.name)
    .toSorted()
    .flatMap((plugin) => checkFile(root, plugin));
}

function main(argv: string[]): number {
  if (argv[0] === "--print") {
    const [plugin = "", event = "", entry = ""] = argv.slice(1);
    process.stdout.write(`${JSON.stringify(launcherHook({ plugin, event, entry }), null, 2)}\n`);
    return 0;
  }
  const rootAt = argv.indexOf("--root");
  const root = resolve(rootAt >= 0 ? (argv[rootAt + 1] ?? ".") : join(import.meta.dir, "../.."));
  const problems = checkHooksJson(root);
  for (const p of problems) {
    process.stderr.write(`${p.file}: ${p.where}: ${p.problem}\n`);
    if (p.expected !== undefined) process.stderr.write(`  expected: ${p.expected}\n`);
  }
  if (problems.length > 0) return 1;
  process.stdout.write("check-hooks-json: ok\n");
  return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
