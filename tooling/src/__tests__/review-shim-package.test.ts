/** The actual OpenCode npm tarball retains the old helper path into native review state. */
import { expect, test } from "bun:test";
import { lstatSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { packInto, stageOpencode } from "../npm-pack.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const BIN = join(
  ROOT,
  "target",
  process.env.TOOLU_IMPL?.startsWith("rust:") ? "release" : "debug",
  "toolu",
);

function reviewRepo(sb: Sandbox): void {
  sb.write(".gitignore", ".opencode/\n");
  sb.write("base.txt", "base\n");
  sb.git("add", "-A");
  sb.git("commit", "-qm", "base");
  sb.git("checkout", "-qb", "feature");
  sb.write("feature.txt", "reviewed change\n");
  sb.git("add", "-A");
  sb.git("commit", "-qm", "feature");
  sb.write(".opencode/toolu.config.json", { version: 1, gates: { preset: "strict" } });
}

async function packedReviewPlugin(sb: Sandbox): Promise<string> {
  const stage = stageOpencode(join(sb.root, "stage"));
  const archive = packInto(stage, sb.root);
  const unpacked = join(sb.root, "unpacked");
  mkdirSync(unpacked);
  const extracted = await run(["tar", "-xf", archive, "-C", unpacked], { cwd: sb.root });
  expect(extracted.exitCode).toBe(0);
  return join(unpacked, "package/plugins/toolu-review");
}

async function publishStableHelper(sb: Sandbox, plugin: string, env: EnvPatch): Promise<string> {
  const started = await run(
    [
      BIN,
      "toolu-review",
      "hook",
      "session-start",
      "--event",
      "SessionStart",
      "--plugin-root",
      plugin,
    ],
    { cwd: sb.project, env, stdin: '{"session_id":"packed-review"}' },
  );
  expect(started.exitCode).toBe(0);
  const stable = join(sb.project, ".opencode/toolu-review/write-state.sh");
  expect(lstatSync(stable).isSymbolicLink()).toBe(true);
  return stable;
}

test("the packed stable helper publishes and writes native v2 state with legacy flags", async () => {
  using sb = createSandbox({ git: true });
  reviewRepo(sb);
  const plugin = await packedReviewPlugin(sb);
  const config = join(sb.project, ".opencode");
  const env: EnvPatch = {
    PATH: `${join(ROOT, "target/debug")}:${process.env.PATH ?? "/usr/bin"}`,
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: config,
    PUSH_REVIEW_BASE: "main",
  };
  const stable = await publishStableHelper(sb, plugin, env);
  const flags = [
    "--reviewers",
    '["packed-review"]',
    "--repo",
    sb.project,
    "--branch",
    "feature",
    "--reviewed-files",
    "feature.txt",
  ];
  const first = await run(
    [stable, "--findings-count", "1", "--findings", '[{"code":"review"}]', ...flags],
    { cwd: sb.project, env },
  );
  const file = join(config, "tmp/push-review/feature.json");
  expect(first).toMatchObject({ exitCode: 0, stdout: `${file}\n` });
  expect(JSON.parse(readFileSync(file, "utf8"))).toMatchObject({
    version: 2,
    branch: "feature",
    base_branch: "main",
    reviewers: ["packed-review"],
    findings_count: 1,
    findings: [{ code: "review" }],
    review_round: 1,
    reviewed_files: ["feature.txt"],
  });
  const second = await run([stable, "--findings-count", "0", "--findings", "[]", ...flags], {
    cwd: sb.project,
    env,
  });
  expect(second.exitCode).toBe(0);
  expect(JSON.parse(readFileSync(file, "utf8"))).toMatchObject({
    findings_count: 0,
    findings: [],
    review_round: 2,
  });
});
