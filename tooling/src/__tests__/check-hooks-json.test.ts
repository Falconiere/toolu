import { afterAll, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherHook } from "@toolu/core/launcher";
import { z } from "zod";
import { checkHooksJson } from "../check-hooks-json.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const CLI = join(ROOT, "tooling/src/check-hooks-json.ts");
const TOOLU = "plugins/toolu/hooks/hooks.json";
const temps: string[] = [];

afterAll(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

/** A temp root holding a copy of every real plugin's hooks.json and committed bundles. */
function copyOfRepo(): string {
  const root = mkdtempSync(join(tmpdir(), "check-hooks-json-"));
  temps.push(root);
  // plugins/ also holds the collection lint config, which is not a plugin.
  const plugins = readdirSync(join(ROOT, "plugins"), { withFileTypes: true })
    .filter((item) => item.isDirectory())
    .map((item) => item.name);
  for (const plugin of plugins) {
    for (const part of ["hooks/hooks.json", "hooks/dist"]) {
      const from = join(ROOT, "plugins", plugin, part);
      try {
        cpSync(from, join(root, "plugins", plugin, part), { recursive: true });
      } catch (error) {
        if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) throw error;
      }
    }
  }
  return root;
}

function edit(root: string, path: string, change: (text: string) => string): void {
  const file = join(root, path);
  const before = readFileSync(file, "utf8");
  const after = change(before);
  expect(after).not.toBe(before);
  writeFileSync(file, after);
}

const sessionStart = launcherHook({
  plugin: "toolu",
  event: "SessionStart",
  entry: "session-start",
});

function cli(...args: string[]) {
  return spawnSync(process.execPath, [CLI, ...args], { encoding: "utf8" });
}

test("the repository's hooks.json files pass", () => {
  expect(checkHooksJson(ROOT)).toEqual([]);
  const result = cli();
  expect(result.status).toBe(0);
});

test("an unchanged copy passes", () => {
  expect(checkHooksJson(copyOfRepo())).toEqual([]);
});

test("a hand-edited launcher command fails and names the expected string", () => {
  const root = copyOfRepo();
  edit(root, TOOLU, (text) =>
    text.replace(
      JSON.stringify(sessionStart.command),
      JSON.stringify(sessionStart.command.replace("exit 0", "exit 1")),
    ),
  );
  const problems = checkHooksJson(root);
  expect(problems).toHaveLength(1);
  expect(problems[0]).toMatchObject({
    file: TOOLU,
    problem: "command differs from the generated launcher",
    expected: sessionStart.command,
  });
  const result = cli("--root", root);
  expect(result.status).toBe(1);
  expect(result.stderr).toContain(`expected: ${sessionStart.command}`);
});

test("an edited or removed commandWindows fails", () => {
  const edited = copyOfRepo();
  edit(edited, TOOLU, (text) =>
    text.replace(
      JSON.stringify(sessionStart.commandWindows),
      JSON.stringify(sessionStart.commandWindows.replace("where /q bun", "where bun")),
    ),
  );
  expect(checkHooksJson(edited).map((p) => p.problem)).toEqual([
    "commandWindows differs from the generated launcher",
  ]);

  const removed = copyOfRepo();
  edit(removed, TOOLU, (text) =>
    text.replace(
      `,\n            "commandWindows": ${JSON.stringify(sessionStart.commandWindows)}`,
      "",
    ),
  );
  expect(checkHooksJson(removed).map((p) => p.problem)).toEqual([
    "commandWindows differs from the generated launcher",
  ]);
});

test("a launcher hook whose bundle is not committed fails", () => {
  const root = copyOfRepo();
  rmSync(join(root, "plugins/toolu/hooks/dist/session-start.js"));
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual([
    "bundle plugins/toolu/hooks/dist/session-start.js is not committed",
  ]);
});

test("commandWindows on a legacy script command fails", () => {
  const root = copyOfRepo();
  edit(root, "plugins/jev/hooks/hooks.json", (text) =>
    text.replace(
      /"command": [^\n]+,\n\s*"commandWindows": [^\n]+/,
      '"command": "\\"${CLAUDE_PLUGIN_ROOT}/hooks/legacy.sh\\"", "commandWindows": "x",',
    ),
  );
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual([
    "launcher hook names no hooks/dist/<entry>.js bundle",
  ]);
});

test("a hand-written bun command fails even though it names a real bundle", () => {
  const root = copyOfRepo();
  const bare = 'bun \\"${CLAUDE_PLUGIN_ROOT}/hooks/dist/session-start.js\\"';
  edit(root, TOOLU, (text) =>
    text.replace(JSON.stringify(sessionStart.command).slice(1, -1), bare),
  );
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual([
    "command differs from the generated launcher",
  ]);
});

test("a hand-written toolu hook command without the generated launcher fails", () => {
  const root = copyOfRepo();
  edit(root, TOOLU, (text) =>
    text
      .replace(JSON.stringify(sessionStart.command), JSON.stringify("toolu hook session-start"))
      .replace(
        `,\n            "commandWindows": ${JSON.stringify(sessionStart.commandWindows)}`,
        "",
      ),
  );
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual(["unsupported native hook command"]);
});

test("an invalid entry name and malformed JSON are reported", () => {
  const badEntry = copyOfRepo();
  edit(badEntry, TOOLU, (text) =>
    text.replaceAll("hooks/dist/session-start.js", "hooks/dist/Session.js"),
  );
  expect(checkHooksJson(badEntry)[0]?.problem).toMatch(/^launcher entry must match/);

  const broken = copyOfRepo();
  edit(broken, TOOLU, (text) => text.slice(0, -3));
  expect(checkHooksJson(broken)[0]?.problem).toMatch(/^invalid hooks\.json/);
});

test("--print emits the generated hook for a new entry", () => {
  const result = cli("--print", "jev", "PreToolUse", "pre-tool-use");
  expect(result.status).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual(
    launcherHook({ plugin: "jev", event: "PreToolUse", entry: "pre-tool-use" }),
  );
});

test("--print rejects missing or invalid arguments with a usage line", () => {
  for (const args of [
    [],
    ["jev", "PreToolUse"],
    ["jev", "PreToolUse", "Bad Entry"],
    ["jev", "", "x"],
  ]) {
    const result = cli("--print", ...args);
    expect(result.status).toBe(2);
    expect(result.stdout).toBe("");
    expect(result.stderr).toStartWith(
      "usage: check-hooks-json.ts --print <plugin> <Event> <entry>\n",
    );
  }
});

/** The native launcher `cargo xtask print-hook` emits, from the committed Rust golden. */
const NATIVE_COMMAND = readFileSync(
  join(ROOT, "crates/core/protocol/src/tests/fixtures/launcher-session-start.txt"),
  "utf8",
);
const NATIVE_PRE_TOOL = readFileSync(
  join(ROOT, "crates/core/protocol/src/tests/fixtures/launcher-pre-tool-use.txt"),
  "utf8",
);
const NativeHook = z.object({
  type: z.literal("command"),
  command: z.string(),
  commandWindows: z.string(),
  timeout: z.number(),
});
const NATIVE_SESSION_HOOK = NativeHook.parse(
  JSON.parse(
    readFileSync(join(ROOT, "tooling/fixtures/native-launcher/session-start.json"), "utf8"),
  ),
);
const NATIVE_PRE_HOOK = NativeHook.parse(
  JSON.parse(
    readFileSync(join(ROOT, "tooling/fixtures/native-launcher/pre-tool-use.json"), "utf8"),
  ),
);

function switchSessionStart(root: string): void {
  const path = join(root, TOOLU);
  const doc = z
    .looseObject({
      hooks: z.looseObject({
        SessionStart: z.array(
          z.looseObject({ hooks: z.array(z.looseObject({ command: z.string() })) }),
        ),
      }),
    })
    .parse(JSON.parse(readFileSync(path, "utf8")));
  const hook = doc.hooks.SessionStart[0]?.hooks[0];
  if (hook === undefined) throw new Error("toolu has no SessionStart hook");
  Object.assign(hook, NATIVE_SESSION_HOOK);
  writeFileSync(path, JSON.stringify(doc));
}

test("a native launcher does not need its transition bundle on disk", () => {
  const root = copyOfRepo();
  switchSessionStart(root);
  rmSync(join(root, "plugins/toolu/hooks/dist/session-start.js"));
  expect(checkHooksJson(root)).toEqual([]);
});

test("a generated native entry passes beside Bun entries and a hand edit fails", () => {
  expect(NATIVE_COMMAND).toContain("--hook-protocol");
  expect(NATIVE_COMMAND).toContain("hooks/dist/session-start.js");
  expect(NATIVE_SESSION_HOOK.command).toBe(NATIVE_COMMAND);
  expect(NATIVE_SESSION_HOOK.commandWindows).toContain("toolu is not installed");
  const root = copyOfRepo();
  switchSessionStart(root);
  expect(checkHooksJson(root)).toEqual([]);
  edit(root, TOOLU, (text) => text.replace("--hook-protocol", "--wrong-protocol"));
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual(["unsupported native hook command"]);
});

test("generated native prompt and pre-compaction entries pass the launcher check", () => {
  const root = copyOfRepo();
  const path = join(root, TOOLU);
  const doc = z
    .looseObject({
      hooks: z.looseObject({
        UserPromptSubmit: z.array(z.looseObject({ hooks: z.array(z.unknown()) })),
        PreCompact: z.array(z.looseObject({ hooks: z.array(z.unknown()) })),
      }),
    })
    .parse(JSON.parse(readFileSync(path, "utf8")));
  for (const [event, filename] of [
    ["UserPromptSubmit", "user-prompt-submit.json"],
    ["PreCompact", "pre-compact.json"],
  ] as const) {
    const hook = doc.hooks[event][0]?.hooks;
    if (hook === undefined || hook.length === 0) throw new Error(`${event} hook missing`);
    hook[0] = NativeHook.parse(
      JSON.parse(readFileSync(join(ROOT, "tooling/fixtures/native-launcher", filename), "utf8")),
    );
  }
  writeFileSync(path, JSON.stringify(doc));
  expect(checkHooksJson(root)).toEqual([]);
});

test("a native command with its protocol marker kept but its shell body edited fails", () => {
  const root = copyOfRepo();
  switchSessionStart(root);
  edit(root, TOOLU, (text) =>
    text.replace(
      JSON.stringify(NATIVE_COMMAND),
      JSON.stringify(NATIVE_COMMAND.replace("exit 0", "exit 1")),
    ),
  );
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual([
    "command differs from the generated native launcher",
  ]);
});

test("a native entry does not stop the Bun launcher check", () => {
  const root = copyOfRepo();
  expect(NATIVE_PRE_HOOK.command).toBe(NATIVE_PRE_TOOL);
  edit(root, TOOLU, (text) =>
    text.replace(
      '"PreToolUse": [',
      `"PreToolUse": [${JSON.stringify({ hooks: [NATIVE_PRE_HOOK] })},`,
    ),
  );
  expect(checkHooksJson(root)).toEqual([]);
  edit(root, TOOLU, (text) =>
    text.replace(
      JSON.stringify(sessionStart.command),
      JSON.stringify(sessionStart.command.replace("exit 0", "exit 1")),
    ),
  );
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual([
    "command differs from the generated launcher",
  ]);
});

test("native Windows and timeout fields must retain the generated values", () => {
  const root = copyOfRepo();
  switchSessionStart(root);
  edit(root, TOOLU, (text) => text.replace("where /q bun", "where bun"));
  expect(checkHooksJson(root).map((p) => p.problem)).toEqual([
    "commandWindows differs from the generated native launcher",
  ]);
  const missingTimeout = copyOfRepo();
  switchSessionStart(missingTimeout);
  edit(missingTimeout, TOOLU, (text) => text.replace('"timeout":60', '"timeout":0'));
  expect(checkHooksJson(missingTimeout).map((p) => p.problem)).toEqual([
    "timeout must be an integer from 1 to 600",
  ]);
});
