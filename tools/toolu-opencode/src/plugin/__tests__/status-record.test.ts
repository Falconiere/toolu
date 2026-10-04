/**
 * The status a settled startup leaves (#359): the real `prepareEnforcement`
 * over the repository catalog writes the record the statusline status skill
 * reads, and sends one structured host-log entry. Neither carries an env value.
 */
import { expect, test } from "bun:test";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { opencodeStatusPath, readOpencodeStatus } from "@toolu/core/startup";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { definedEnv } from "../../host/runtime-env.ts";
import type { HostBinding, LogExtra, LogLevel } from "../context.ts";
import { createTooluHooks } from "../hooks.ts";
import { hook } from "./jev-fixtures.ts";

const FAKE_SECRET = "sk-test-359";

type Entry = { level: LogLevel; message: string; extra?: LogExtra };

function select(sb: Sandbox, enabled: readonly string[]): void {
  mkdirSync(join(sb.project, ".opencode/toolu"), { recursive: true });
  writeFileSync(
    join(sb.project, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled }),
  );
}

function hostBinding(sb: Sandbox, entries: Entry[], repoRoot = REPO_ROOT): HostBinding {
  const env = definedEnv(process.env);
  for (const name of [
    "TOOLU_REPO_ROOT",
    "TOOLU_ROOT",
    "TOOLU_CONFIG_DIR",
    "TOOLU_OPENCODE_HOME",
    "TYPESAFE_API_KEY",
  ])
    delete env[name];
  Object.assign(env, {
    HOME: sb.home,
    XDG_CONFIG_HOME: join(sb.home, ".config"),
    TOOLU_BUN: process.execPath,
    TOOLU_FAKE_SECRET: FAKE_SECRET,
  });
  return {
    directory: sb.project,
    worktree: sb.project,
    projectRoot: sb.project,
    repoRootOption: repoRoot,
    optionsError: undefined,
    env,
    log: (level, message, extra) => {
      entries.push(extra === undefined ? { level, message } : { level, message, extra });
      return Promise.resolve();
    },
  };
}

function recordPath(sb: Sandbox): string {
  return opencodeStatusPath(join(sb.project, ".opencode/toolu/state"));
}

function statusEntries(entries: readonly Entry[]): Entry[] {
  return entries.filter((entry) => entry.message === "toolu: status");
}

test("a ready startup records the selected plugins and logs one structured status entry", async () => {
  using sb = createSandbox();
  select(sb, ["statusline", "jev"]);
  const entries: Entry[] = [];
  const hooks = await createTooluHooks(hostBinding(sb, entries));
  try {
    const path = recordPath(sb);
    const read = readOpencodeStatus(path);
    if (!read.ok) throw new Error(`record ${read.reason}`);
    const { record } = read;
    expect(record).toMatchObject({ version: 1, project: sb.project, status: "ready" });
    expect(record.selection).toBe("project");
    expect(record.plugins).toEqual([
      { name: "jev", entries: ["session-start"], artifacts: 1 },
      { name: "statusline", entries: ["session-start"], artifacts: 1 },
    ]);
    expect(Date.now() - Date.parse(record.written)).toBeLessThan(120_000);
    expect(statusEntries(entries)).toEqual([
      {
        level: "info",
        message: "toolu: status",
        extra: {
          status: "ready",
          plugins: "jev,statusline",
          artifacts: 2,
          record: path,
          selection: "project",
        },
      },
    ]);
    expect(entries.at(-1)?.message).toBe("toolu: status");
    // The bash env shell.env builds names the directory the status skill reads.
    const output: { env: Record<string, string> } = { env: {} };
    await hook(hooks, "shell.env")({ cwd: sb.project, sessionID: "s", callID: "c" }, output);
    expect(opencodeStatusPath(output.env.TOOLU_CONFIG_DIR ?? "")).toBe(path);
    expect(readFileSync(path, "utf8")).not.toContain(FAKE_SECRET);
    expect(JSON.stringify(entries)).not.toContain(FAKE_SECRET);
  } finally {
    await hooks.dispose?.();
  }
});

test("a not-ready startup records and logs the bounded bootstrap reason", async () => {
  using sb = createSandbox();
  const catalog = sb.path("catalog");
  cpSync(join(REPO_ROOT, "plugins"), join(catalog, "plugins"), { recursive: true });
  rmSync(join(catalog, "plugins/jev/hooks/dist/session-start.js"));
  select(sb, ["statusline", "jev"]);
  const entries: Entry[] = [];
  const hooks = await createTooluHooks(hostBinding(sb, entries, catalog));
  try {
    const read = readOpencodeStatus(recordPath(sb));
    if (!read.ok) throw new Error(`record ${read.reason}`);
    expect(read.record).toMatchObject({ status: "not-ready", plugins: [], notes: [] });
    expect(read.record.reason).toStartWith("bootstrap: jev: session-start: ");
    expect(read.record.selection).toBeUndefined();
    const [status] = statusEntries(entries);
    expect(status?.level).toBe("error");
    expect(status?.extra).toMatchObject({ status: "not-ready", plugins: "", artifacts: 0 });
    expect(status?.extra?.reason).toBe(read.record.reason);
    expect(JSON.stringify(entries)).not.toContain(FAKE_SECRET);
  } finally {
    await hooks.dispose?.();
  }
});

test("a record that cannot be written is logged and leaves the verdict ready", async () => {
  using sb = createSandbox();
  select(sb, ["statusline"]);
  // A non-empty directory at the record path: the rename over it fails.
  const path = recordPath(sb);
  mkdirSync(join(path, "occupied"), { recursive: true });
  const entries: Entry[] = [];
  const hooks = await createTooluHooks(hostBinding(sb, entries));
  try {
    const failures = entries.filter((entry) =>
      entry.message.startsWith(`toolu: status record not written: cannot write ${path}: `),
    );
    expect(failures).toHaveLength(1);
    expect(failures[0]?.level).toBe("error");
    expect(statusEntries(entries)[0]?.extra?.status).toBe("ready");
    const before = hook(hooks, "tool.execute.before");
    const args = { command: "echo ok", description: "x" };
    await before({ tool: "bash", sessionID: "s", callID: "c" }, { args });
    expect(readOpencodeStatus(path)).toEqual({ ok: false, reason: "invalid" });
  } finally {
    await hooks.dispose?.();
  }
});
