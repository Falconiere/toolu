import { expect, test } from "bun:test";
import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
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
test.concurrent("readList parses every shipped list file", () => {
  const lists = readdirSync(SETTINGS).filter((name) => name.endsWith(".txt"));
  expect(lists.length).toBe(6);
  for (const name of lists) {
    expect(readList(join(SETTINGS, name))).toEqual(expect.any(Array));
  }
});

test.concurrent("readList keeps lines verbatim and drops comments and blanks", () => {
  using sb = createSandbox();
  const path = sb.write(
    "list.txt",
    "# comment\n   # indented comment\n\n \t \nvalue # not a comment\n  spaced  \nlast-without-newline",
  );
  const expected = ["value # not a comment", "  spaced  ", "last-without-newline"];
  expect(readList(path)).toEqual(expected);
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

/** Lines and the prefix/redirect `mcp-blocker.sh` split them into (verified against it before #260 deleted it). */
const MCP_LINES: readonly (readonly [string, string, string])[] = [
  [
    "claude_ai_Atlassian -> use the host's native Jira tools instead",
    "claude_ai_Atlassian",
    "use the host's native Jira tools instead",
  ],
  ["  figma  ", "figma", ""],
  ["canva -> a -> b", "canva", "a -> b"],
];

for (const [line, prefix, redirect] of MCP_LINES) {
  test.concurrent(`mcpBlocklist splits "${line}" as mcp-blocker.sh did`, () => {
    using sb = createSandbox();
    const dir = join(sb.root, "settings");
    mkdirSync(dir);
    writeFileSync(join(dir, "mcp-blocklist.txt"), `# header\n${line}\n`);
    expect(mcpBlocklist(dir)).toEqual([{ prefix, redirect }]);
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

test.concurrent("settingsDir: TOOLU_SETTINGS_DIR, then ~/.claude/settings", () => {
  using sb = createSandbox();
  const explicit = { HOME: sb.home, TOOLU_SETTINGS_DIR: "/opt/toolu-settings" };
  expect(settingsDir({ env: explicit })).toBe("/opt/toolu-settings");
  mkdirSync(join(sb.home, ".claude", "settings"), { recursive: true });
  const legacy = { HOME: sb.home };
  expect(settingsDir({ env: legacy })).toBe(join(sb.home, ".claude", "settings"));
});

test.concurrent("settingsDir falls back to <plugin root>/settings, else undefined", () => {
  using sb = createSandbox();
  expect(settingsDir({ env: { HOME: sb.home, CLAUDE_PLUGIN_ROOT: "/p/toolu" } })).toBe(
    "/p/toolu/settings",
  );
  expect(settingsDir({ env: { HOME: sb.home }, pluginRoot: "/q" })).toBe("/q/settings");
  expect(settingsDir({ env: { HOME: sb.home } })).toBeUndefined();
});
