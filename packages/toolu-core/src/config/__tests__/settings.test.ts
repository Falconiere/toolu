import { expect, test } from "bun:test";
import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import {
  bashAllowlist,
  bashDenylist,
  codeEditRules,
  commitPrefixes,
  mcpBlocklist,
  protectedFiles,
  readList,
  rustUnsafeExemptions,
  settingsDir,
} from "../settings.ts";

const REPO = resolve(import.meta.dir, "../../../../..");
const SETTINGS = join(REPO, "plugins/toolu/settings");
const LIB = join(REPO, "plugins/toolu/hooks/lib");
const MCP_BLOCKER = join(REPO, "plugins/toolu/hooks/pre-tools/modules/mcp-blocker.sh");

async function bashReadList(path: string): Promise<string[]> {
  const res = await run(["bash", "-c", '. "$1/detect.sh"; read_list "$2"', "_", LIB, path]);
  // grep exits 1 when every line is a comment; callers only read stdout.
  expect([0, 1]).toContain(res.exitCode);
  return res.stdout === "" ? [] : res.stdout.replace(/\n$/, "").split("\n");
}

test.concurrent("readList matches bash read_list on every shipped list file", async () => {
  const lists = readdirSync(SETTINGS).filter((name) => name.endsWith(".txt"));
  expect(lists.length).toBe(6);
  for (const name of lists) {
    expect(readList(join(SETTINGS, name))).toEqual(await bashReadList(join(SETTINGS, name)));
  }
});

test.concurrent("readList keeps lines verbatim and drops comments and blanks like grep", async () => {
  using sb = createSandbox();
  const path = sb.write(
    "list.txt",
    "# comment\n   # indented comment\n\n \t \nvalue # not a comment\n  spaced  \nlast-without-newline",
  );
  const expected = ["value # not a comment", "  spaced  ", "last-without-newline"];
  expect(readList(path)).toEqual(expected);
  expect(await bashReadList(path)).toEqual(expected);
  expect(readList(join(sb.project, "absent.txt"))).toEqual([]);
});

test.concurrent("the typed list loaders read the shipped files", () => {
  expect(bashDenylist(SETTINGS)).toEqual([
    "node -e",
    "node -p",
    "node --eval",
    "node --print",
    "bun -e",
    "bun --eval",
    "cargo test",
  ]);
  expect(bashAllowlist(SETTINGS)).toEqual([]);
  expect(commitPrefixes(SETTINGS)).toEqual([
    "feat",
    "fix",
    "chore",
    "docs",
    "refactor",
    "test",
    "perf",
    "build",
    "ci",
    "style",
    "revert",
  ]);
  expect(protectedFiles(SETTINGS)).toContain(".env");
  expect(rustUnsafeExemptions(SETTINGS)).toEqual([]);
  expect(mcpBlocklist(SETTINGS)).toEqual([]);
});

const MCP_LINES: readonly (readonly [string, string])[] = [
  ["claude_ai_Atlassian -> use the `jira` skill instead", "claude_ai_AtlassianCloud"],
  ["  figma  ", "figmaDesktop"],
  ["canva -> a -> b", "canva"],
];

for (const [line, server] of MCP_LINES) {
  test.concurrent(`mcpBlocklist splits "${line}" like mcp-blocker.sh`, async () => {
    using sb = createSandbox();
    const dir = join(sb.root, "settings");
    mkdirSync(dir);
    writeFileSync(join(dir, "mcp-blocklist.txt"), `# header\n${line}\n`);
    const [entry] = mcpBlocklist(dir);
    expect(entry).toBeDefined();
    if (entry === undefined) return;
    expect(server.startsWith(entry.prefix)).toBe(true);
    sb.writeConfig("claude", "project", { gates: { mcpBlocker: { mode: "block" } } });
    const res = await run(["bash", MCP_BLOCKER], {
      env: {
        HOME: sb.home,
        TOOLU_PROJECT_DIR: sb.project,
        TOOLU_SETTINGS_DIR: dir,
        TOOLU_HOST_OVERRIDE: "claude",
        tool_name: `mcp__${server}__call`,
      },
    });
    const reason = String(JSON.parse(res.stdout).hookSpecificOutput.permissionDecisionReason);
    expect(reason).toContain(`MCP server "${server}" is blocked`);
    const hint = reason.split(" Use instead: ")[1] ?? "";
    expect(hint).toBe(entry.redirect);
  });
}

test.concurrent("mcpBlocklist drops an entry whose prefix is empty", () => {
  using sb = createSandbox();
  const path = sb.write("s/mcp-blocklist.txt", "  -> hint only\n\tok \n");
  expect(mcpBlocklist(join(path, ".."))).toEqual([{ prefix: "ok", redirect: "" }]);
});

test.concurrent("codeEditRules parses the shipped file into camelCase rules", () => {
  const result = codeEditRules(SETTINGS);
  expect(result.ok).toBe(true);
  if (!result.ok) return;
  const ts = result.rules.find((rule) => rule.match === "*.ts");
  expect(ts?.whenPathMatches).toContain("*/components/*");
  expect(ts?.extraDocs.length).toBeGreaterThan(0);
  expect(result.rules.find((rule) => rule.match === "*.rs")?.whenPathMatches).toEqual([]);
});

test.concurrent("codeEditRules: absent is empty, malformed and off-schema are errors", () => {
  using sb = createSandbox();
  expect(codeEditRules(sb.root)).toEqual({ ok: true, rules: [] });
  const bad = sb.write("bad/code-edit-rules.json", '{"rules": [');
  expect(codeEditRules(join(bad, "..")).ok).toBe(false);
  const off = sb.write(
    "off/code-edit-rules.json",
    JSON.stringify({ rules: [{ match: "*.ts", doc: [] }] }),
  );
  const result = codeEditRules(join(off, ".."));
  expect(result.ok).toBe(false);
  if (!result.ok) expect(result.reason).toContain("code-edit-rules.json");
});

async function bashSettingsDir(env: Record<string, string>): Promise<string> {
  const res = await run(["bash", "-c", '. "$1/detect.sh"; toolu_settings_dir', "_", LIB], { env });
  return res.stdout.trim();
}

test.concurrent("settingsDir: TOOLU_SETTINGS_DIR, then ~/.claude/settings, like bash", async () => {
  using sb = createSandbox();
  const explicit = { HOME: sb.home, TOOLU_SETTINGS_DIR: "/opt/toolu-settings" };
  expect(settingsDir({ env: explicit })).toBe("/opt/toolu-settings");
  expect(await bashSettingsDir(explicit)).toBe("/opt/toolu-settings");
  mkdirSync(join(sb.home, ".claude", "settings"), { recursive: true });
  const legacy = { HOME: sb.home };
  expect(settingsDir({ env: legacy })).toBe(join(sb.home, ".claude", "settings"));
  expect(await bashSettingsDir(legacy)).toBe(join(sb.home, ".claude", "settings"));
});

test.concurrent("settingsDir falls back to <plugin root>/settings, else undefined", () => {
  using sb = createSandbox();
  expect(settingsDir({ env: { HOME: sb.home, CLAUDE_PLUGIN_ROOT: "/p/toolu" } })).toBe(
    "/p/toolu/settings",
  );
  expect(settingsDir({ env: { HOME: sb.home }, pluginRoot: "/q" })).toBe("/q/settings");
  expect(settingsDir({ env: { HOME: sb.home } })).toBeUndefined();
});
