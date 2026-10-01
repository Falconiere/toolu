import { describe, expect, test } from "bun:test";
import { registryEventFor } from "../registry-types.ts";
import {
  parseRegistryName,
  registryEventDir,
  registryFileName,
  registryRoot,
} from "../registry-paths.ts";

const ENVS: Record<string, { env: Record<string, string>; root: string }> = {
  claude: { env: { HOME: "/home/u" }, root: "/home/u/.claude/toolu" },
  claudeConfigDir: { env: { HOME: "/home/u", CLAUDE_CONFIG_DIR: "/cc" }, root: "/cc/toolu" },
  codex: { env: { HOME: "/home/u", PLUGIN_ROOT: "/p", CODEX_HOME: "/cx" }, root: "/cx/toolu" },
  override: {
    env: { HOME: "/home/u", TOOLU_CONFIG_DIR: "/t", CLAUDE_CONFIG_DIR: "/cc" },
    root: "/t/toolu",
  },
};

describe("registryEventDir", () => {
  for (const [label, { env, root }] of Object.entries(ENVS)) {
    test.concurrent(`resolves event directories (${label})`, () => {
      expect(registryEventDir("tool/pre", { env })).toBe(`${root}/pre-tools.d`);
      expect(registryEventDir("tool/post", { env })).toBe(`${root}/post-tools.d`);
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
