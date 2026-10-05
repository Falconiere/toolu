import { expect, test } from "bun:test";
import { contractPaths } from "../../opencode-host/results.ts";
import { ProbeResultsSchema, readJson } from "../../opencode-host/schema.ts";
import { acceptanceChecks } from "../families.ts";
import { refusePresetPackage, restoreKeys, selection, setAsideKeys } from "../run.ts";

// What a run sets up before any host session (#362): selection, credentials and the package override.

const registry = acceptanceChecks(readJson(contractPaths().results, ProbeResultsSchema));

test.concurrent("service credentials leave the environment and come back only to the caller", () => {
  const env: Record<string, string | undefined> = {
    TYPESAFE_API_KEY: "sk-362",
    JIRA_PAT: "pat-362",
    PATH: "/usr/bin",
  };
  expect(setAsideKeys(env)).toEqual({ TYPESAFE_API_KEY: "sk-362", JIRA_PAT: "pat-362" });
  expect(env).toEqual({ PATH: "/usr/bin" });
});

test.concurrent("a package override left in the shell is refused", () => {
  expect(() => refusePresetPackage({ TOOLU_ACCEPTANCE_PACKAGE: "/tmp/other" })).toThrow(
    "unset TOOLU_ACCEPTANCE_PACKAGE (/tmp/other)",
  );
  expect(() => refusePresetPackage({ TOOLU_ACCEPTANCE_PACKAGE: "" })).not.toThrow();
  expect(() => refusePresetPackage({})).not.toThrow();
});

test.concurrent("a complete run selects every check and every control", () => {
  const all = selection(registry, []);
  expect(all.checks).toHaveLength(registry.length);
  expect(all.controls).toHaveLength(4);
});

test.concurrent("--only selects named checks and controls apart", () => {
  const onlyControl = selection(registry, ["control.v2-entry"]);
  expect(onlyControl.checks).toEqual([]);
  expect(onlyControl.controls.map((control) => control.id)).toEqual(["control.v2-entry"]);
  const mixed = selection(registry, ["control.v2-entry", "entry.npm-root"]);
  expect(mixed.checks.map((check) => check.id)).toEqual(["entry.npm-root"]);
  expect(() => selection(registry, ["control.v2-entry", "nope.missing"])).toThrow(
    "unknown acceptance check: nope.missing",
  );
});

test.concurrent("credentials set aside come back after the run", () => {
  const env: Record<string, string | undefined> = { TYPESAFE_API_KEY: "sk-362", PATH: "/usr/bin" };
  const kept = setAsideKeys(env);
  expect(env.TYPESAFE_API_KEY).toBeUndefined();
  restoreKeys(kept, env);
  expect(env).toEqual({ TYPESAFE_API_KEY: "sk-362", PATH: "/usr/bin" });
});
