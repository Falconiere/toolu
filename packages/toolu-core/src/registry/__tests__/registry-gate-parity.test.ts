/**
 * `pluginActive` vs bash `toolu_plugin_active` (#257, AC-2). Each case writes
 * the same install record or Codex snapshot into a fresh sandbox, then asks
 * both implementations about the same spec.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { pluginActive, pluginPresence } from "../registry-gate.ts";

const DETECT_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/detect.sh");
const SPEC = "ts-quality@toolu";

type Case = {
  label: string;
  host: "claude" | "codex";
  spec?: string;
  /** Relative to the sandbox home; `null` body = a directory. */
  file?: { path: string; body: string | null };
  env?: (sb: Sandbox) => EnvPatch;
  expected: boolean;
};

const CLAUDE_FILE = ".claude/plugins/installed_plugins.json";
const SNAPSHOT = "codex/toolu/codex-plugins.json";
const plugins = (value: unknown) => JSON.stringify({ version: 2, plugins: value });
const snapshot = (status: string, list: unknown) =>
  JSON.stringify({ version: 1, status, plugins: list });

const CASES: Case[] = [
  {
    label: "claude: installed",
    host: "claude",
    file: { path: CLAUDE_FILE, body: plugins({ [SPEC]: [{}] }) },
    expected: true,
  },
  {
    label: "claude: null value still installed",
    host: "claude",
    file: { path: CLAUDE_FILE, body: plugins({ [SPEC]: null }) },
    expected: true,
  },
  {
    label: "claude: absent",
    host: "claude",
    file: { path: CLAUDE_FILE, body: plugins({ "other@toolu": [] }) },
    expected: false,
  },
  { label: "claude: record missing", host: "claude", expected: true },
  {
    label: "claude: malformed JSON",
    host: "claude",
    file: { path: CLAUDE_FILE, body: "{nope" },
    expected: true,
  },
  {
    label: "claude: empty file",
    host: "claude",
    file: { path: CLAUDE_FILE, body: "" },
    expected: true,
  },
  {
    label: "claude: plugins is an array",
    host: "claude",
    file: { path: CLAUDE_FILE, body: plugins([SPEC]) },
    expected: true,
  },
  {
    label: "claude: record is a directory",
    host: "claude",
    file: { path: CLAUDE_FILE, body: null },
    expected: true,
  },
  {
    label: "claude: empty spec",
    host: "claude",
    spec: "",
    file: { path: CLAUDE_FILE, body: plugins({ "": [] }) },
    expected: false,
  },
  {
    label: "claude: CLAUDE_PLUGINS_REGISTRY wins",
    host: "claude",
    file: { path: "elsewhere.json", body: plugins({}) },
    env: (sb) => ({ CLAUDE_PLUGINS_REGISTRY: join(sb.home, "elsewhere.json") }),
    expected: false,
  },
  {
    label: "claude: CLAUDE_CONFIG_DIR root",
    host: "claude",
    file: { path: "cc/plugins/installed_plugins.json", body: plugins({}) },
    env: (sb) => ({ CLAUDE_CONFIG_DIR: join(sb.home, "cc") }),
    expected: false,
  },
  {
    label: "codex: ready and listed",
    host: "codex",
    file: { path: SNAPSHOT, body: snapshot("ready", [SPEC]) },
    expected: true,
  },
  {
    label: "codex: ready and not listed",
    host: "codex",
    file: { path: SNAPSHOT, body: snapshot("ready", []) },
    expected: false,
  },
  {
    label: "codex: indeterminate",
    host: "codex",
    file: { path: SNAPSHOT, body: snapshot("indeterminate", []) },
    expected: true,
  },
  { label: "codex: snapshot missing", host: "codex", expected: true },
  {
    label: "codex: malformed snapshot",
    host: "codex",
    file: { path: SNAPSHOT, body: "[]" },
    expected: true,
  },
];

function arrange(sb: Sandbox, c: Case): EnvPatch {
  if (c.file !== undefined) {
    const abs = join(sb.home, c.file.path);
    if (c.file.body === null) {
      mkdirSync(abs, { recursive: true });
    } else {
      mkdirSync(dirname(abs), { recursive: true });
      writeFileSync(abs, c.file.body);
    }
  }
  const hostEnv: EnvPatch =
    c.host === "codex"
      ? { PLUGIN_ROOT: sb.path("plugin"), CODEX_HOME: join(sb.home, "codex") }
      : {};
  return { HOME: sb.home, ...hostEnv, ...c.env?.(sb) };
}

async function bashActive(env: EnvPatch, spec: string): Promise<boolean> {
  const res = await run(["bash", "-c", '. "$1"; toolu_plugin_active "$2"', "_", DETECT_SH, spec], {
    env,
  });
  expect(res.stderr).toBe("");
  return res.exitCode === 0;
}

for (const c of CASES) {
  test.concurrent(`matches bash toolu_plugin_active: ${c.label}`, async () => {
    using sb = createSandbox();
    const env = arrange(sb, c);
    const spec = c.spec ?? SPEC;
    const hostEnv = Object.fromEntries(
      Object.entries(env).filter((kv): kv is [string, string] => kv[1] !== undefined),
    );
    const ts = pluginActive(spec, { env: hostEnv });
    expect(ts).toBe(c.expected);
    expect(await bashActive(env, spec)).toBe(ts);
  });
}

test.concurrent("hosts without a readable install record are unknown, so modules stay active", () => {
  using sb = createSandbox();
  writeFileSync(join(sb.home, "installed_plugins.json"), plugins({}));
  const env = { HOME: sb.home, CLAUDE_PLUGINS_REGISTRY: join(sb.home, "installed_plugins.json") };
  for (const host of ["cursor", "hermes", "opencode"] as const) {
    expect(pluginPresence(SPEC, { env, host })).toBe("unknown");
    expect(pluginActive(SPEC, { env, host })).toBe(true);
  }
  expect(pluginPresence(SPEC, { env, host: "claude" })).toBe("absent");
});
