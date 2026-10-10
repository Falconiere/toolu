import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { definedEnv } from "../../host/runtime-env.ts";
import { astGrepRule } from "../ast-grep-native.ts";

test("a native ast-grep rule that ignores SIGTERM still times out", async () => {
  using sb = createSandbox();
  const binary = sb.write(
    "slow-native",
    "#!/usr/bin/env bun\nprocess.on('SIGTERM', () => {});\nawait Bun.sleep(120_000);\n",
  );
  chmodSync(binary, 0o755);
  const configRoot = sb.path("config");
  const manifest = join(configRoot, "toolu/pre-tools.d/ast-grep@toolu__search-nudge.json");
  mkdirSync(dirname(manifest), { recursive: true });
  writeFileSync(manifest, "{}\n");

  const result = await astGrepRule(
    "pre-tools",
    { tool_name: "Grep" },
    {
      configRoot,
      cwd: sb.project,
      env: { ...definedEnv(process.env), TOOLU_BIN: binary },
    },
  );
  expect(result).toEqual({
    exitCode: 1,
    stdout: "",
    stderr: "ast-grep native rule timed out after 30000 ms",
  });
});
