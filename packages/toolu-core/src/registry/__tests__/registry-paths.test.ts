import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { childEnv } from "@toolu/conformance/harness/spawn";
import { registryEventFor } from "../registry-types.ts";
import {
  parseRegistryName,
  registryEventDir,
  registryFileName,
  registryRoot,
} from "../registry-paths.ts";

const REGISTRY_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/registry.sh");

/** `toolu_registry_event_dir EVENT` from the real bash lib under `env`. */
function bashEventDir(event: string, env: Record<string, string | undefined>): string {
  const res = spawnSync(
    "bash",
    ["-c", '. "$1"; toolu_registry_event_dir "$2"', "_", REGISTRY_SH, event],
    { env: childEnv(env), encoding: "utf8" },
  );
  expect(res.stderr).toBe("");
  return res.stdout;
}

const ENVS: Record<string, Record<string, string | undefined>> = {
  claude: { HOME: "/home/u" },
  claudeConfigDir: { HOME: "/home/u", CLAUDE_CONFIG_DIR: "/cc" },
  codex: { HOME: "/home/u", PLUGIN_ROOT: "/p", CODEX_HOME: "/cx" },
  override: { HOME: "/home/u", TOOLU_CONFIG_DIR: "/t", CLAUDE_CONFIG_DIR: "/cc" },
};

describe("registryEventDir", () => {
  for (const [label, env] of Object.entries(ENVS)) {
    test.concurrent(`matches bash toolu_registry_event_dir (${label})`, () => {
      expect(registryEventDir("tool/pre", { env })).toBe(bashEventDir("PreToolUse", env));
      expect(registryEventDir("tool/post", { env })).toBe(bashEventDir("PostToolUse", env));
    });
  }

  test.concurrent("the root is <config root>/toolu", () => {
    expect(registryRoot({ env: { HOME: "/home/u" }, host: "claude" })).toBe(
      "/home/u/.claude/toolu",
    );
  });
});

describe("registryEventFor", () => {
  test.concurrent("shell/pre dispatches from the pre-tool directory", () => {
    expect(registryEventFor("shell/pre")).toBe("tool/pre");
    expect(registryEventFor("tool/pre")).toBe("tool/pre");
    expect(registryEventFor("tool/post")).toBe("tool/post");
  });
});

describe("registryFileName", () => {
  test.concurrent("joins spec and name with the double underscore", () => {
    expect(registryFileName("ts-quality@toolu", "ts-quality")).toBe(
      "ts-quality@toolu__ts-quality.js",
    );
    expect(registryFileName("x@git.example.com", "a.b")).toBe("x@git.example.com__a.b.js");
  });

  test.concurrent.each([
    ["", "n"],
    ["a b", "n"],
    ["a__b", "n"],
    ["a/b", "n"],
    ["s", ""],
    ["s", "a/b"],
    ["s", ".hidden"],
  ])("rejects spec %p name %p", (spec, name) => {
    expect(() => registryFileName(spec, name)).toThrow(TypeError);
  });

  test.concurrent("round-trips through parseRegistryName", () => {
    expect(parseRegistryName(registryFileName("a@b", "c__d"))).toEqual({
      spec: "a@b",
      name: "c__d",
      kind: "esm",
    });
  });
});
