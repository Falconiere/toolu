import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { JsonObject, LoadedConfig } from "../config-load.ts";
import {
  nativeMaxLines,
  qualityFlag,
  qualityThreshold,
  tsMaxFileLinesResolved,
} from "../quality-config.ts";

function config(data: JsonObject): LoadedConfig {
  return {
    data,
    invalid: undefined,
    files: { user: "/u", project: undefined },
    host: "claude",
    warn: () => {},
  };
}

const EMPTY = config({});

test.concurrent("overrides win when they are positive numbers or numeric strings", () => {
  const c = config({
    lang: {
      ts: { maxFileLines: " 120 ", maxFnLines: 0 },
      rust: { maxFileLines: -5, maxFnLines: 45.9, maxImplLines: "+250" },
      python: { maxFileLines: "off", maxFnLines: true },
    },
  });
  const root = "/nonexistent-root";
  expect(qualityThreshold(c, "ts", "maxFileLines", { root })).toBe(120);
  expect(qualityThreshold(c, "ts", "maxFnLines", { root })).toBe(60);
  expect(qualityThreshold(c, "rust", "maxFileLines")).toBe(500);
  expect(qualityThreshold(c, "rust", "maxFnLines")).toBe(45);
  expect(qualityThreshold(c, "rust", "maxImplLines")).toBe(250);
  expect(qualityThreshold(c, "python", "maxFileLines")).toBe(400);
  expect(qualityThreshold(c, "python", "maxFnLines")).toBe(50);
});

test.concurrent("a sub-1 override floors to 0, as bash prints it", () => {
  expect(
    qualityThreshold(config({ lang: { rust: { maxFnLines: 0.5 } } }), "rust", "maxFnLines"),
  ).toBe(0);
});

test.concurrent("nativeMaxLines reads every eslint/oxlint encoding", () => {
  const rules = (rule: unknown) => ({ rules: { "max-lines": rule } });
  expect(nativeMaxLines(rules(150))).toBe(150);
  expect(nativeMaxLines(rules(["error", 250]))).toBe(250);
  expect(nativeMaxLines(rules(["warn", { max: "200", skipBlankLines: true }]))).toBe(200);
  expect(nativeMaxLines(rules([2, 90.7]))).toBe(90);
  expect(nativeMaxLines(rules(["off", 100]))).toBeUndefined();
  expect(nativeMaxLines(rules([0, 100]))).toBeUndefined();
  expect(nativeMaxLines(rules(["error"]))).toBeUndefined();
  expect(nativeMaxLines(rules("error"))).toBeUndefined();
  expect(nativeMaxLines(rules(["error", { max: 0 }]))).toBeUndefined();
  expect(nativeMaxLines([rules(1)])).toBeUndefined();
});

test.concurrent("native layer reads the active linter only: oxc over eslint", () => {
  using sb = createSandbox({
    git: true,
    files: {
      ".oxlintrc.json": JSON.stringify({ rules: { "max-lines": ["error", { max: 222 }] } }),
      ".eslintrc.json": JSON.stringify({ rules: { "max-lines": ["error", 111] } }),
    },
  });
  expect(tsMaxFileLinesResolved(EMPTY, { cwd: sb.project })).toEqual({
    value: 222,
    source: "native",
  });
  expect(qualityThreshold(EMPTY, "ts", "maxFileLines", { cwd: sb.project })).toBe(222);
  // maxFnLines never reads the linter.
  expect(qualityThreshold(EMPTY, "ts", "maxFnLines", { cwd: sb.project })).toBe(60);
});

test.concurrent("biome, flat eslint and non-JSON configs fall through to the default", () => {
  using biome = createSandbox({
    git: true,
    files: {
      "biome.json": "{}",
      ".oxlintrc.json": JSON.stringify({ rules: { "max-lines": 99 } }),
    },
  });
  expect(tsMaxFileLinesResolved(EMPTY, { cwd: biome.project })).toEqual({
    value: 300,
    source: "default",
  });
  using flat = createSandbox({ git: true, files: { "eslint.config.mjs": "export default [];" } });
  expect(tsMaxFileLinesResolved(EMPTY, { cwd: flat.project }).source).toBe("default");
  using broken = createSandbox({ git: true, files: { ".oxlintrc.json": "{ // comment\n}" } });
  expect(tsMaxFileLinesResolved(EMPTY, { cwd: broken.project }).source).toBe("default");
});

test.concurrent("an override beats the native layer; outside git the default applies", () => {
  using sb = createSandbox({
    git: true,
    files: { ".eslintrc.json": JSON.stringify({ rules: { "max-lines": 111 } }) },
  });
  const c = config({ lang: { ts: { maxFileLines: 80 } } });
  expect(tsMaxFileLinesResolved(c, { cwd: sb.project })).toEqual({ value: 80, source: "override" });
  expect(tsMaxFileLinesResolved(EMPTY, { cwd: sb.project })).toEqual({
    value: 111,
    source: "native",
  });
  using bare = createSandbox();
  expect(tsMaxFileLinesResolved(EMPTY, { cwd: bare.project }).source).toBe("default");
});

test.concurrent("qualityFlag honors only a JSON boolean", () => {
  const c = config({ lang: { ts: { noMocks: false }, rust: { noMocks: "false" }, python: [] } });
  expect(qualityFlag(c, "ts", "noMocks", true)).toBe(false);
  expect(qualityFlag(c, "rust", "noMocks", true)).toBe(true);
  expect(qualityFlag(c, "python", "noMocks", true)).toBe(true);
});
