import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  agentArgs as epicArgs,
  HOST_LIMIT,
  parseHostKind,
} from "../../../../epic-orchestrator/scripts/hosts";
import { DEFAULT_TABLE } from "../../../../epic-orchestrator/scripts/route";
import { agentArgs, hostKind, loadFixerConfig } from "../babysit/fixer-route.ts";

const name = "pb-3fa2c1-r1g1";

test("native fixer argv matches the epic host contract for 24 combinations", () => {
  const models = { claude: "opus", codex: "gpt-6-sol", cursor: "gpt-5.6-sol-high" };
  let count = 0;
  for (const kind of ["claude", "codex", "cursor"] as const)
    for (const bypass of [true, false])
      for (const model of [null, models[kind]])
        for (const effort of [null, "high"]) {
          expect(agentArgs(kind, name, model, effort, bypass)).toEqual(
            epicArgs(kind, {
              key: name,
              model: model ?? undefined,
              effort: effort ?? undefined,
              bypass,
              permissionMode: "auto",
              resume: false,
            }),
          );
          count += 1;
        }
  expect(count).toBe(24);
});

test("native config defaults match epic routing table", () => {
  const temp = mkdtempSync(join(tmpdir(), "babysit-config-"));
  const oldConfig = process.env.TOOLU_CONFIG_DIR;
  const oldProject = process.env.TOOLU_PROJECT_DIR;
  process.env.TOOLU_CONFIG_DIR = temp;
  process.env.TOOLU_PROJECT_DIR = temp;
  try {
    const config = loadFixerConfig("claude");
    expect(config.routing).toEqual({
      claude: DEFAULT_TABLE.hosts.claude!,
      codex: DEFAULT_TABLE.hosts.codex!,
      cursor: DEFAULT_TABLE.hosts.cursor!,
      opencode: DEFAULT_TABLE.hosts.opencode!,
    });
    expect(config.dispatch).toBe("herdr");
  } finally {
    if (oldConfig === undefined) delete process.env.TOOLU_CONFIG_DIR;
    else process.env.TOOLU_CONFIG_DIR = oldConfig;
    if (oldProject === undefined) delete process.env.TOOLU_PROJECT_DIR;
    else process.env.TOOLU_PROJECT_DIR = oldProject;
    rmSync(temp, { recursive: true, force: true });
  }
});

test("host aliases and usage limit wording stay aligned", () => {
  for (const alias of ["cursor-agent", "Claude-Code", " Cursor-Agent ", "codex", "OpenCode"])
    expect(hostKind(alias)).toBe(
      parseHostKind(alias) as "claude" | "codex" | "cursor" | "opencode",
    );
  expect(HOST_LIMIT.test("rate limit reached")).toBe(true);
  expect(HOST_LIMIT.test("all tests passed")).toBe(false);
});

test("shell unsafe model identifiers are rejected before launch", () => {
  for (const [kind, model] of [
    ["claude", "a b"],
    ["cursor", "claude[effort=high]"],
  ] as const)
    expect(() => agentArgs(kind, name, model, null, true)).toThrow("unsafe");
});
