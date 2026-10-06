/**
 * hooks.json launcher gate (#250). Every hook that starts a Bun bundle must carry
 * exactly the `command` / `commandWindows` pair `@toolu/core/launcher` generates,
 * so no plugin can hand-edit the fail-closed contract away.
 *
 * A hook is a launcher hook when its command names `hooks/dist/`, runs
 * `bun`, or carries `commandWindows`. Legacy script commands pass untouched until
 * their plugin is ported (epic #247). Native entries are checked against the
 * #412 Rust command goldens; `cargo xtask check-hooks` checks every native field.
 *
 * Usage: bun run tooling/src/check-hooks-json.ts [--root <dir>]
 *        bun run tooling/src/check-hooks-json.ts --print <plugin> <event> <entry>
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { isEnforcingEvent, launcherHook, type LauncherHook } from "@toolu/core/launcher";
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
    timeout: z.number().optional(),
  })
  .passthrough();
const HooksFileSchema = z
  .object({
    hooks: z.record(z.string(), z.array(z.object({ hooks: z.array(HookSchema) }).passthrough())),
  })
  .passthrough();

type Hook = z.infer<typeof HookSchema>;

const DIST = /hooks[/\\]dist[/\\]([^"'\s/\\]+)\.js/;

/** The marker of a native launcher entry (`toolu_protocol::launcher::MARKER`). */
const NATIVE_MARKER = "--hook-protocol";
const NATIVE_EXAMPLES = {
  enforcing: {
    event: "PreToolUse",
    entry: "pre-tools",
    command: readFileSync(
      join(
        import.meta.dir,
        "../../crates/core/protocol/src/tests/fixtures/launcher-pre-tool-use.txt",
      ),
      "utf8",
    ),
  },
  context: {
    event: "SessionStart",
    entry: "session-start",
    command: readFileSync(
      join(
        import.meta.dir,
        "../../crates/core/protocol/src/tests/fixtures/launcher-session-start.txt",
      ),
      "utf8",
    ),
  },
} as const;

function expectedHook(plugin: string, event: string, entry: string): LauncherHook | string {
  try {
    return launcherHook({ plugin, event, entry });
  } catch (error) {
    if (error instanceof Error) return error.message;
    throw error;
  }
}

/** The Rust generator's checked-in command, with only its target fields changed. */
function expectedNativeCommand(plugin: string, event: string, entry: string): string | undefined {
  if (!/^[A-Z][A-Za-z]+$/u.test(event) || typeof expectedHook(plugin, event, entry) === "string")
    return undefined;
  const example = isEnforcingEvent(event) ? NATIVE_EXAMPLES.enforcing : NATIVE_EXAMPLES.context;
  const prefix = plugin === "toolu" ? "" : `${plugin} `;
  return example.command
    .replace(
      `hook ${example.entry} --event ${example.event}`,
      `${prefix}hook ${entry} --event ${event}`,
    )
    .replaceAll(`hooks/dist/${example.entry}.js`, `hooks/dist/${entry}.js`)
    .replaceAll("toolu plugin:", `${plugin} plugin:`);
}

/** The context golden carries the #412 install text used by both POSIX and Windows. */
function nativeMissingMessage(plugin: string): string {
  const message = /"systemMessage":"(toolu plugin: [^"]+ is not installed[^"]*)"/u.exec(
    NATIVE_EXAMPLES.context.command,
  )?.[1];
  if (message === undefined) throw new Error("native SessionStart golden has no install message");
  return message.replace("toolu plugin:", `${plugin} plugin:`);
}

function expectedNativeWindows(plugin: string, event: string, entry: string): string {
  const bundle = `"%PLUGIN_ROOT%\\hooks\\dist\\${entry}.js"`;
  const home = '"%USERPROFILE%\\.bun\\bin\\bun.exe"';
  const missing = nativeMissingMessage(plugin).replaceAll("|", "^|");
  const absent = isEnforcingEvent(event)
    ? `(1>&2 echo blocked: ${missing}& exit /b 2)`
    : `(echo {"systemMessage":"${missing}"})`;
  return `if exist "%TOOLU_BUN%" ("%TOOLU_BUN%" ${bundle}) else (where /q bun& if not errorlevel 1 (bun ${bundle}) else if exist ${home} (${home} ${bundle}) else ${absent})`;
}

function isNativeHook(hook: Hook): boolean {
  return (hook.command ?? "").includes(NATIVE_MARKER);
}

function isLauncherHook(hook: Hook): boolean {
  const command = hook.command ?? "";
  return (
    hook.commandWindows !== undefined || command.includes("hooks/dist/") || /\bbun\b/u.test(command)
  );
}

/** A hand-written native call must fail even when its generated marker is missing. */
export function isNativeLikeCommand(command: string): boolean {
  return (
    /\btoolu(?:\s+[a-z0-9-]+)?\s+hook\s+[a-z0-9-]+/u.test(command) ||
    /\bhook\s+[a-z0-9-]+\s+--event\s+[A-Z][A-Za-z]+/u.test(command)
  );
}

function entryOf(hook: Hook): string | undefined {
  return (DIST.exec(hook.command ?? "") ?? DIST.exec(hook.commandWindows ?? ""))?.[1];
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

function checkNativeHook(site: HookSite, hook: Hook): HooksJsonProblem[] {
  const { file, where } = site;
  const entry = entryOf(hook);
  if (entry === undefined)
    return [{ file, where, problem: "native launcher names no hooks/dist/<entry>.js bundle" }];
  const expected = expectedNativeCommand(site.plugin, site.event, entry);
  if (expected === undefined) return [{ file, where, problem: "invalid native launcher target" }];
  const problems: HooksJsonProblem[] = [];
  if (hook.type !== "command")
    problems.push({
      file,
      where,
      problem: `type must be "command", got ${JSON.stringify(hook.type)}`,
    });
  if (hook.command !== expected)
    problems.push({
      file,
      where,
      problem: "command differs from the generated native launcher",
      expected,
    });
  const windows = expectedNativeWindows(site.plugin, site.event, entry);
  if (hook.commandWindows !== windows)
    problems.push({
      file,
      where,
      problem: "commandWindows differs from the generated native launcher",
      expected: windows,
    });
  if (
    hook.timeout === undefined ||
    !Number.isInteger(hook.timeout) ||
    hook.timeout < 1 ||
    hook.timeout > 600
  )
    problems.push({ file, where, problem: "timeout must be an integer from 1 to 600" });
  const bundle = `plugins/${site.plugin}/hooks/dist/${entry}.js`;
  if (!existsSync(join(site.root, bundle)))
    problems.push({ file, where, problem: `bundle ${bundle} is not committed` });
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
        isNativeHook(hook)
          ? checkNativeHook(
              { root, plugin, event, file, where: `${event}[${i}].hooks[${j}]` },
              hook,
            )
          : isNativeLikeCommand(hook.command ?? "")
            ? [
                {
                  file,
                  where: `${event}[${i}].hooks[${j}]`,
                  problem: "unsupported native hook command",
                },
              ]
            : isLauncherHook(hook)
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
    const hook = event === "" ? "launcher event is required" : expectedHook(plugin, event, entry);
    if (typeof hook === "string") {
      process.stderr.write(
        `usage: check-hooks-json.ts --print <plugin> <Event> <entry>\n${hook}\n`,
      );
      return 2;
    }
    process.stdout.write(`${JSON.stringify(hook, null, 2)}\n`);
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
