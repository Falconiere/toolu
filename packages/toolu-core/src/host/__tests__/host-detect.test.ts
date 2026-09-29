import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { detectHost } from "../host-detect.ts";
import type { HostEnv } from "../host-name.ts";

function detect(env: HostEnv, extra: { hookEventName?: string; inProcess?: "opencode" } = {}) {
  const warnings: string[] = [];
  const host = detectHost({ env, ...extra, warn: (line) => warnings.push(line) });
  return { host, warnings };
}

describe("detectHost", () => {
  test("defaults to Claude with no host signal", () => {
    expect(detect({ HOME: "/home/u" })).toEqual({ host: "claude", warnings: [] });
  });

  test("Codex PLUGIN_ROOT wins over the Claude compatibility variables", () => {
    const env = { PLUGIN_ROOT: "/codex", CLAUDE_PLUGIN_ROOT: "/compat" };
    expect(detect(env).host).toBe("codex");
  });

  test("Cursor's per-hook variables win over PLUGIN_ROOT", () => {
    expect(detect({ CURSOR_VERSION: "1.7.0", PLUGIN_ROOT: "/p" }).host).toBe("cursor");
    expect(detect({ CURSOR_PROJECT_DIR: "/repo", CLAUDE_PROJECT_DIR: "/repo" }).host).toBe(
      "cursor",
    );
  });

  test("HERMES_HOME alone never selects Hermes", () => {
    expect(detect({ HERMES_HOME: "/home/u/.hermes" }).host).toBe("claude");
  });

  test("a host-unique stdin hook_event_name selects its host", () => {
    expect(detect({}, { hookEventName: "pre_tool_call" }).host).toBe("hermes");
    expect(detect({ PLUGIN_ROOT: "/p" }, { hookEventName: "beforeShellExecution" }).host).toBe(
      "cursor",
    );
  });

  test("a PascalCase event name is shared and falls through to environment signals", () => {
    expect(detect({ PLUGIN_ROOT: "/p" }, { hookEventName: "PreToolUse" }).host).toBe("codex");
    expect(detect({}, { hookEventName: "PreToolUse" }).host).toBe("claude");
    expect(detect({}, { hookEventName: "made_up_event" }).host).toBe("claude");
  });

  test("the in-process OpenCode flag selects OpenCode over environment signals", () => {
    expect(detect({ PLUGIN_ROOT: "/p" }, { inProcess: "opencode" }).host).toBe("opencode");
  });

  test("TOOLU_HOST_OVERRIDE accepts every host and wins over all signals", () => {
    for (const host of ["claude", "codex", "cursor", "opencode", "hermes"] as const) {
      const env = { TOOLU_HOST_OVERRIDE: host, PLUGIN_ROOT: "/p", CURSOR_VERSION: "1" };
      expect(detect(env, { hookEventName: "pre_tool_call" })).toEqual({ host, warnings: [] });
    }
  });

  test("an invalid override warns once and falls back to detection", () => {
    expect(detect({ TOOLU_HOST_OVERRIDE: "gemini", PLUGIN_ROOT: "/p" })).toEqual({
      host: "codex",
      warnings: ["toolu-host: invalid TOOLU_HOST_OVERRIDE 'gemini' (using environment detection)"],
    });
  });

  test("empty variables count as unset", () => {
    expect(detect({ TOOLU_HOST_OVERRIDE: "", PLUGIN_ROOT: "", CURSOR_VERSION: "" })).toEqual({
      host: "claude",
      warnings: [],
    });
  });

  test("defaults to process.env and writes the invalid-override warning to stderr", () => {
    const script = `import { detectHost } from ${JSON.stringify(join(import.meta.dir, "../host.ts"))}; process.stdout.write(detectHost());`;
    const res = spawnSync(process.execPath, ["-e", script], {
      encoding: "utf8",
      env: { PATH: process.env.PATH ?? "", TOOLU_HOST_OVERRIDE: "bogus", PLUGIN_ROOT: "/p" },
    });
    expect(res.status).toBe(0);
    expect(res.stdout).toBe("codex");
    expect(res.stderr).toBe(
      "toolu-host: invalid TOOLU_HOST_OVERRIDE 'bogus' (using environment detection)\n",
    );
  });
});
