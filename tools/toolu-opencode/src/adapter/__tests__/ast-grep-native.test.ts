import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { definedEnv } from "../../host/runtime-env.ts";
import { astGrepRule, type NativeRuleOptions } from "../ast-grep-native.ts";

function nativeRuleOptions(sb: Sandbox, script: string): NativeRuleOptions {
  const binary = sb.write("native-rule", script);
  chmodSync(binary, 0o755);
  const configRoot = sb.path("config");
  const manifest = join(configRoot, "toolu/pre-tools.d/ast-grep@toolu__search-nudge.json");
  mkdirSync(dirname(manifest), { recursive: true });
  writeFileSync(manifest, "{}\n");
  return { configRoot, cwd: sb.project, env: { ...definedEnv(process.env), TOOLU_BIN: binary } };
}

test("a native ast-grep rule preserves a normal subprocess result", async () => {
  using sb = createSandbox();
  const options = nativeRuleOptions(sb, "#!/usr/bin/env bun\nconsole.log('ready');\n");
  const result = await astGrepRule("pre-tools", { tool_name: "Grep" }, options);
  expect(result).toEqual({ exitCode: 0, stdout: "ready\n", stderr: "" });
});

test("a native ast-grep rule that ignores SIGTERM still times out", async () => {
  using sb = createSandbox();
  const options = nativeRuleOptions(
    sb,
    "#!/usr/bin/env bun\nprocess.on('SIGTERM', () => {});\nawait Bun.sleep(120_000);\n",
  );
  const result = await astGrepRule("pre-tools", { tool_name: "Grep" }, options);
  expect(result).toEqual({
    exitCode: 1,
    stdout: "",
    stderr: "ast-grep native rule timed out after 30000 ms",
  });
});
