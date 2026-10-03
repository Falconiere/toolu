/**
 * context7 on OpenCode (#348): the real context7 and toolu bundles through the
 * adapter's hooks, and the published helper run the way the agent's bash runs
 * it (the `shell.env` environment) against a loopback HTTPS fixture posing as
 * context7.com. The key comes from the environment only, never a project `.env`.
 */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Config } from "@opencode-ai/plugin";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { startHttpsFixture } from "@toolu/conformance/https-fixture";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { isPlainRecord } from "../../surfaces/merge.ts";
import type { HostBinding } from "../context.ts";
import { createTooluHooks } from "../hooks.ts";
import { bashEnv, binding, hook, inShell, systemLines, type Logged } from "./jev-fixtures.ts";

const GENERATED = realpathSync(join(REPO_ROOT, "tools/toolu-opencode/generated"));
const INSTRUCTION = "context7 (library docs)";
const SKILL = 'Syntax: skill({ name: "context7-context7" }).';
// The ctx7sk_ prefix is load-bearing, but a literal ctx7sk_<random> reads as a
// live credential to secret scanners: assemble it from an inert suffix.
const PREFIX = "ctx7sk_";
const KEY = `${PREFIX}env-key`;
const SECRET = `${PREFIX}dotenv-secret`;
const LIBRARIES = { results: [{ id: "/facebook/react", title: "React" }] };
const DOCS = { codeSnippets: [{ codeTitle: "useEffect" }], infoSnippets: [] };
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

/** A project (optionally under a directory with a space) selecting `enabled`. */
function project(sb: Sandbox, enabled: string[], dir = "."): string {
  const root = join(sb.project, dir);
  mkdirSync(join(root, ".opencode/toolu"), { recursive: true });
  writeFileSync(
    join(root, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled }),
  );
  writeFileSync(join(root, ".env"), `CONTEXT7_API_KEY=${SECRET}\n`);
  return root;
}

function bindingAt(sb: Sandbox, root: string, logged: Logged[]): HostBinding {
  const base = binding(sb, logged, "");
  base.env.CLAUDE_CONFIG_DIR = join(sb.home, ".claude");
  delete base.env.CONTEXT7_API_KEY;
  return { ...base, directory: root, worktree: root, projectRoot: root };
}

function count(lines: readonly string[], text: string): number {
  return lines.filter((line) => line.includes(text)).length;
}

function runCommand(context: string): string {
  const command = context.split("you MUST run ")[1]?.split(" FIRST")[0];
  if (command === undefined) throw new Error(`no command in: ${context}`);
  return command;
}

/** The generated skill's OpenCode command, with its placeholder arguments. */
function skillCommand(): string {
  const skill = readFileSync(join(GENERATED, "skills/context7-context7/SKILL.md"), "utf8");
  const line = skill.split("# OpenCode\n")[1]?.split("\n")[0];
  if (line === undefined) throw new Error("no # OpenCode command in the generated skill");
  return line.replace(" <command> [options]", "");
}

test.concurrent("one instruction per request, none in compaction, and the native skill", async () => {
  using sb = createSandbox();
  const root = project(sb, ["toolu", "context7"]);
  const hooks = await createTooluHooks(bindingAt(sb, root, []));
  try {
    const helper = join(root, ".opencode/toolu/state/context7/search.sh");
    const requests = await Promise.all(
      ["ses_a", "ses_b", "ses_a"].map((sessionID) => systemLines(hooks, sessionID)),
    );
    for (const lines of requests) {
      expect(count(lines, INSTRUCTION)).toBe(1);
      const line = lines.find((text) => text.includes(INSTRUCTION)) ?? "";
      expect(runCommand(line)).toBe(`${quote(process.execPath)} --no-env-file ${quote(helper)}`);
      expect(line).toContain(SKILL);
    }
    const compaction = { context: ["keep-me"] };
    await hook(hooks, "experimental.session.compacting")({ sessionID: "ses_a" }, compaction);
    expect(compaction.context).toContain("toolu sessionID: ses_a");
    expect(count(compaction.context, INSTRUCTION)).toBe(0);
    const config: Config = {};
    await hook(hooks, "config")(config);
    const skills: unknown = Reflect.get(config, "skills");
    expect(isPlainRecord(skills) ? skills.paths : undefined).toContain(
      join(GENERATED, "skills/context7-context7"),
    );
  } finally {
    await hooks.dispose?.();
  }
});

test.concurrent("search, docs and a 429 through the instruction and skill commands", async () => {
  using sb = createSandbox();
  const root = project(sb, ["toolu", "context7"], "my project");
  const logged: Logged[] = [];
  const fixture = await startHttpsFixture(["context7.com"]);
  const hooks = await createTooluHooks(bindingAt(sb, root, logged));
  try {
    const system = await systemLines(hooks, "ses_a");
    const command = runCommand(system.find((line) => line.includes(INSTRUCTION)) ?? "");
    // Bun's directory is dropped from PATH: both commands name Bun themselves.
    const env = {
      ...(await bashEnv(hooks, sb)),
      ...fixture.env,
      PATH: "/usr/bin:/bin",
      CONTEXT7_API_KEY: undefined,
    };
    const inRoot = (line: string) => inShell({ ...sb, project: root }, line, env);

    fixture.plan([{ body: JSON.stringify(LIBRARIES) }]);
    const search = await inRoot(`${command} search react`);
    expect(search).toMatchObject({ exitCode: 0, stderr: "" });
    expect(JSON.parse(search.stdout)).toEqual(LIBRARIES);
    expect(fixture.requests[0]?.path).toBe("/api/v2/libs/search?libraryName=react&query=react");
    expect(fixture.requests[0]?.headers.authorization).toBeUndefined();

    fixture.plan([{ body: JSON.stringify(DOCS) }]);
    const docs = await inRoot(`${command} docs /facebook/react "useEffect cleanup"`);
    expect(docs).toMatchObject({ exitCode: 0, stderr: "" });
    expect(JSON.parse(docs.stdout)).toEqual(DOCS);
    expect(fixture.requests[0]?.path).toBe(
      "/api/v2/context?libraryId=%2Ffacebook%2Freact&query=useEffect%20cleanup&type=json",
    );

    fixture.plan([{ status: 429, body: '{"error":"rate limited"}' }]);
    const limited = await inRoot(`${command} search react`);
    expect(limited.exitCode).toBe(22);
    expect(limited.stderr).toBe(
      "context7: HTTP 429 from https://context7.com/api/v2/libs/search?libraryName=react&query=react\n",
    );

    fixture.plan([{ body: JSON.stringify(LIBRARIES) }]);
    const viaSkill = await inRoot(`${skillCommand()} search react`);
    expect(viaSkill).toMatchObject({ exitCode: 0, stderr: "" });
    expect(fixture.requests[0]?.path).toBe("/api/v2/libs/search?libraryName=react&query=react");

    const seen = [...system, ...logged.map((line) => line.message)].join("\n");
    expect(seen).not.toContain(SECRET);
  } finally {
    await hooks.dispose?.();
    await fixture.stop();
  }
});

test.concurrent("a project .env key is never sent; an environment key is", async () => {
  using sb = createSandbox();
  const root = project(sb, ["toolu", "context7"]);
  const fixture = await startHttpsFixture(["context7.com"]);
  const hooks = await createTooluHooks(bindingAt(sb, root, []));
  try {
    const system = await systemLines(hooks, "ses_a");
    const command = runCommand(system.find((line) => line.includes(INSTRUCTION)) ?? "");
    const helper = join(root, ".opencode/toolu/state/context7/search.sh");
    const env = { ...(await bashEnv(hooks, sb)), ...fixture.env };
    const sent = async (line: string, key: string | undefined) => {
      fixture.plan([{ body: JSON.stringify(LIBRARIES) }]);
      const res = await inShell({ ...sb, project: root }, line, {
        ...env,
        CONTEXT7_API_KEY: key,
      });
      expect(res.exitCode).toBe(0);
      return fixture.requests[0]?.headers.authorization;
    };

    expect(await sent(`${command} search react`, undefined)).toBeUndefined();
    expect(await sent(`${command} search react`, KEY)).toBe(`Bearer ${KEY}`);
    // The hazard the instruction avoids: through its shebang Bun loads the .env.
    expect(await sent(`${quote(helper)} search react`, undefined)).toBe(`Bearer ${SECRET}`);
  } finally {
    await hooks.dispose?.();
    await fixture.stop();
  }
});

test.concurrent("deselected context7 gives no instruction, whatever Claude Code records", async () => {
  using sb = createSandbox();
  const root = project(sb, ["toolu"]);
  // A user-owned helper and a Claude Code record would satisfy toolu's own context7 line.
  const helper = join(root, ".opencode/toolu/state/context7/search.sh");
  mkdirSync(join(helper, ".."), { recursive: true });
  writeFileSync(helper, "#!/bin/sh\necho mine\n");
  chmodSync(helper, 0o755);
  mkdirSync(join(sb.home, ".claude/plugins"), { recursive: true });
  writeFileSync(
    join(sb.home, ".claude/plugins/installed_plugins.json"),
    JSON.stringify({ version: 2, plugins: { "context7@toolu": [{ scope: "user" }] } }),
  );
  const hooks = await createTooluHooks(bindingAt(sb, root, []));
  try {
    expect(count(await systemLines(hooks, "ses_a"), "context7")).toBe(0);
    const compaction: { context: string[] } = { context: [] };
    await hook(hooks, "experimental.session.compacting")({ sessionID: "ses_a" }, compaction);
    expect(count(compaction.context, "context7")).toBe(0);
  } finally {
    await hooks.dispose?.();
  }
});
