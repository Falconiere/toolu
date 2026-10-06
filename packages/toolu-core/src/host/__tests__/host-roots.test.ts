/** Host-native roots and invocation parity over shared JSON cases. */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { materializeCaseValue, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import {
  configRoot,
  invocation,
  pluginData,
  pluginInstallCommand,
  pluginRoot,
  projectConfigPath,
  projectDirname,
  projectRoot,
  projectStateDir,
  projectStateRoot,
} from "../host-roots.ts";

const FnSchema = z.enum([
  "configRoot",
  "projectRoot",
  "projectConfigPath",
  "projectStateRoot",
  "projectStateDir",
  "projectDirname",
  "pluginRoot",
  "pluginData",
  "invocation",
  "pluginInstallCommand",
]);
const CallsSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("calls"),
  git: z.boolean(),
  dirs: z.array(z.string()),
  checks: z
    .array(
      z.strictObject({
        fn: FnSchema,
        args: z.array(z.json()),
        expected: z.json().optional(),
        throws: z.literal("TypeError").optional(),
      }),
    )
    .min(1),
});
const WarningSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("warning"),
  override: z.string(),
  projectDir: z.string(),
  warning: z.string(),
});
const OptionsSchema = z.strictObject({
  env: z.record(z.string(), z.string()).optional(),
  host: z.enum(["claude", "codex", "cursor", "hermes", "opencode"]).optional(),
  cwd: z.string().optional(),
  root: z.string().optional(),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/host/root.json"));

function runtimeValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(runtimeValue);
  if (typeof value !== "object" || value === null) return value;
  const record = z.record(z.string(), z.unknown()).parse(value);
  if (Object.keys(record).length === 1 && record.$runtime === "PATH")
    return process.env.PATH ?? "/usr/bin:/bin";
  if (Object.keys(record).length === 1 && record.$runtime === "OS_CLAUDE_HOME")
    return join(homedir(), ".claude");
  return Object.fromEntries(Object.entries(record).map(([key, item]) => [key, runtimeValue(item)]));
}

function call(sb: Sandbox, fn: z.infer<typeof FnSchema>, raw: readonly unknown[]): unknown {
  const args = raw.map((value) => materializeCaseValue(sb, runtimeValue(value)));
  const options = (value: unknown) => {
    const parsed = OptionsSchema.parse(value);
    return {
      ...(parsed.env === undefined ? {} : { env: parsed.env }),
      ...(parsed.host === undefined ? {} : { host: parsed.host }),
      ...(parsed.cwd === undefined ? {} : { cwd: parsed.cwd }),
      ...(parsed.root === undefined ? {} : { root: parsed.root }),
    };
  };
  const o = () => options(args[0]);
  switch (fn) {
    case "configRoot":
      return configRoot(o());
    case "projectRoot":
      return projectRoot(o());
    case "projectConfigPath":
      return projectConfigPath(o());
    case "projectStateRoot":
      return projectStateRoot(o());
    case "projectDirname":
      return projectDirname(o());
    case "pluginRoot":
      return pluginRoot(o());
    case "pluginData":
      return pluginData(o());
    case "projectStateDir":
      return projectStateDir(z.string().parse(args[0]), options(args[1]));
    case "invocation":
      return invocation(z.string().parse(args[0]), z.string().parse(args[1]), options(args[2]));
    case "pluginInstallCommand":
      return pluginInstallCommand(z.string().parse(args[0]), options(args[1]));
  }
}

for (const raw of cases) {
  if (raw.kind === "calls") {
    const c = CallsSchema.parse(raw);
    test(c.name, () => {
      using sb = createSandbox({ git: c.git });
      for (const dir of c.dirs) {
        const path = dir.startsWith("$ROOT/") ? join(sb.root, dir.slice(6)) : sb.path(dir);
        mkdirSync(path, { recursive: true });
      }
      for (const check of c.checks) {
        if (check.throws !== undefined) {
          expect(() => call(sb, check.fn, check.args)).toThrow(TypeError);
        } else {
          const expected = materializeCaseValue(sb, runtimeValue(check.expected));
          expect<unknown>(call(sb, check.fn, check.args) ?? null).toEqual(expected);
        }
      }
    });
  } else if (raw.kind === "warning") {
    const c = WarningSchema.parse(raw);
    test(c.name, () => {
      using sb = createSandbox();
      const host = join(import.meta.dir, "../host.ts");
      const script = `import { projectConfigPath, codexPluginSnapshotPath } from ${JSON.stringify(host)};\nprojectConfigPath(); process.stderr.write("--\\n"); codexPluginSnapshotPath();`;
      const res = spawnSync(process.execPath, ["-e", script], {
        encoding: "utf8",
        env: {
          PATH: process.env.PATH ?? "/usr/bin:/bin",
          HOME: sb.home,
          TOOLU_HOST_OVERRIDE: c.override,
          TOOLU_PROJECT_DIR: c.projectDir,
        },
      });
      expect(res.status).toBe(0);
      expect(res.stderr).toBe(`${c.warning}\n--\n${c.warning}\n`);
    });
  }
}
