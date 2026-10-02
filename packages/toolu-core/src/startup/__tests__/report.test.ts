/**
 * The startup report channel (#342): real register and publish runs append
 * one JSON record per contribution only when `TOOLU_STARTUP_REPORT` names a
 * file, and a record that cannot be written makes the entry exit non-zero.
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { publishWrapper } from "../publish.ts";

const BUNDLE = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/dist/sample.js");
const FIXTURE = resolve(import.meta.dir, "../../registry/__tests__/fixtures/register-fixture.ts");
const STDIN = JSON.stringify({ hook_event_name: "SessionStart", source: "startup" });

function records(path: string): unknown[] {
  return readFileSync(path, "utf8")
    .split("\n")
    .filter((line) => line !== "")
    .map((line): unknown => JSON.parse(line));
}

async function buildEntry(outDir: string): Promise<string> {
  const result = await Bun.build({ entrypoints: [FIXTURE], target: "bun", outdir: outDir });
  expect(result.success).toBe(true);
  return join(outDir, "register-fixture.js");
}

test.concurrent("a register entry reports written, then unchanged, then failed", async () => {
  using sb = createSandbox();
  const entry = await buildEntry(sb.path("dist"));
  const report = sb.path("report.jsonl");
  const env = {
    HOME: sb.home,
    TOOLU_CONFIG_DIR: sb.path("cfg"),
    FIXTURE_BUNDLE: BUNDLE,
    TOOLU_STARTUP_REPORT: report,
  };
  const target = sb.path("cfg/toolu/post-tools.d/fixture@toolu__fixture.js");
  const base = { kind: "registry", spec: "fixture@toolu", name: "fixture", event: "tool/post" };

  expect(await run([process.execPath, entry], { env, stdin: STDIN })).toMatchObject({
    exitCode: 0,
    stdout: "",
  });
  await run([process.execPath, entry], { env, stdin: STDIN });
  const missing = sb.path("absent.js");
  await run([process.execPath, entry], { env: { ...env, FIXTURE_BUNDLE: missing }, stdin: STDIN });

  expect(records(report)).toEqual([
    { ...base, source: BUNDLE, target, status: "written" },
    { ...base, source: BUNDLE, target, status: "unchanged" },
    {
      ...base,
      source: missing,
      target,
      status: "failed",
      error: expect.stringMatching(/^bundle unreadable/),
    },
  ]);
});

test.concurrent("a register entry that throws reports an error record", async () => {
  using sb = createSandbox();
  const entry = await buildEntry(sb.path("dist"));
  const report = sb.path("report.jsonl");
  const res = await run([process.execPath, entry], {
    env: {
      HOME: sb.home,
      TOOLU_CONFIG_DIR: sb.path("cfg"),
      FIXTURE_BUNDLE: BUNDLE,
      FIXTURE_NAME: "../escape",
      TOOLU_STARTUP_REPORT: report,
    },
    stdin: STDIN,
  });
  expect(res.exitCode).toBe(0);
  expect(records(report)).toEqual([
    {
      kind: "error",
      origin: "fixture@toolu",
      message: 'registry: invalid module name "../escape"',
    },
  ]);
});

test.concurrent("without TOOLU_STARTUP_REPORT nothing is reported and output is unchanged", async () => {
  using sb = createSandbox();
  const entry = await buildEntry(sb.path("dist"));
  const env = { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg"), FIXTURE_BUNDLE: BUNDLE };
  const before = readdirSync(sb.root).toSorted();
  const res = await run([process.execPath, entry], { env, stdin: STDIN });
  expect(res).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
  expect(readdirSync(sb.root).toSorted()).toEqual(before);
  expect(readdirSync(sb.path("cfg/toolu/post-tools.d"))).toEqual(["fixture@toolu__fixture.js"]);
});

test.concurrent("an unwritable report makes the entry exit 1 after it registered", async () => {
  using sb = createSandbox();
  const entry = await buildEntry(sb.path("dist"));
  const report = sb.path("report-is-a-directory");
  mkdirSync(report);
  const res = await run([process.execPath, entry], {
    env: {
      HOME: sb.home,
      TOOLU_CONFIG_DIR: sb.path("cfg"),
      FIXTURE_BUNDLE: BUNDLE,
      TOOLU_STARTUP_REPORT: report,
    },
    stdin: STDIN,
  });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain(`toolu-startup: cannot write startup report ${report}`);
  expect(existsSync(sb.path("cfg/toolu/post-tools.d/fixture@toolu__fixture.js"))).toBe(true);
});

test.concurrent("publishWrapper reports every outcome with its path", () => {
  using sb = createSandbox();
  const report = sb.path("report.jsonl");
  const source = sb.write("plugin/hooks/dist/search.js", "#!/usr/bin/env bun\n");
  const env = { HOME: sb.home, TOOLU_CONFIG_DIR: sb.path("cfg"), TOOLU_STARTUP_REPORT: report };
  const options = { plugin: "context7", dir: "context7", name: "search.sh", env };
  publishWrapper({ ...options, source });
  publishWrapper({ ...options, source: sb.path("absent.js") });
  const userFile = sb.path("cfg/context7/own.sh");
  writeFileSync(userFile, "#!/bin/sh\n");
  publishWrapper({ ...options, name: "own.sh", source });

  const base = { kind: "helper", plugin: "context7" };
  expect(records(report)).toEqual([
    { ...base, source, path: sb.path("cfg/context7/search.sh"), status: "published" },
    { ...base, source: sb.path("absent.js"), status: "source-missing" },
    { ...base, source, path: userFile, status: "kept-user-file" },
  ]);
});
