/**
 * Where the user-level config is (#343): `TOOLU_USER_CONFIG_DIR` names its
 * directory when set; otherwise every host reads it from its config root, as
 * before.
 */
import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { configFiles } from "../config-files.ts";
import { loadConfig } from "../config-load.ts";

const HOSTS = [
  ["claude", ".claude"],
  ["codex", ".codex"],
] as const;

for (const [host, dir] of HOSTS) {
  test.concurrent(`${host}: without TOOLU_USER_CONFIG_DIR the user config stays in the config root`, () => {
    using sb = createSandbox();
    const env = { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_USER_CONFIG_DIR: "" };
    expect(configFiles({ env, host }).files.user).toBe(join(sb.home, dir, "toolu.config.json"));
  });
}

test.concurrent("TOOLU_USER_CONFIG_DIR wins over TOOLU_CONFIG_DIR for the user config only", () => {
  using sb = createSandbox();
  const global = join(sb.root, "global config");
  const data = join(sb.root, "data");
  const env = {
    HOME: sb.home,
    TOOLU_PROJECT_DIR: sb.project,
    TOOLU_CONFIG_DIR: data,
    TOOLU_USER_CONFIG_DIR: global,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".claude",
  };
  const { files } = configFiles({ env, host: "claude" });
  expect(files.user).toBe(join(global, "toolu.config.json"));
  expect(files.project).toBe(join(sb.project, ".claude", "toolu.config.json"));
});

test.concurrent("loadConfig reads the user file from TOOLU_USER_CONFIG_DIR, project still wins", () => {
  using sb = createSandbox();
  const global = join(sb.root, "global config");
  mkdirSync(global, { recursive: true });
  writeFileSync(
    join(global, "toolu.config.json"),
    JSON.stringify({
      version: 1,
      gates: { protectedFiles: { mode: "block" }, qualityGate: { mode: "advise" } },
    }),
  );
  mkdirSync(join(sb.project, ".claude"), { recursive: true });
  writeFileSync(
    join(sb.project, ".claude", "toolu.config.json"),
    JSON.stringify({ version: 1, gates: { protectedFiles: { mode: "off" } } }),
  );
  // A decoy at the data root must not be read.
  const data = join(sb.root, "data");
  mkdirSync(data, { recursive: true });
  writeFileSync(
    join(data, "toolu.config.json"),
    JSON.stringify({ version: 1, hooks: { decoy: false } }),
  );
  const env = {
    HOME: sb.home,
    TOOLU_PROJECT_DIR: sb.project,
    TOOLU_CONFIG_DIR: data,
    TOOLU_USER_CONFIG_DIR: global,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".claude",
  };
  const config = loadConfig({ env, host: "claude", warn: () => {} });
  expect(config.data["gates"]).toEqual({
    protectedFiles: { mode: "off" },
    qualityGate: { mode: "advise" },
  });
  expect(config.data["hooks"]).toBeUndefined();
});
