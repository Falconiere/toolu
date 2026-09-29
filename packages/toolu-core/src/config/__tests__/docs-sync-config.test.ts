import { expect, test } from "bun:test";
import type { JsonObject, LoadedConfig } from "../config-load.ts";
import {
  DOCS_SYNC_DEFAULTS,
  docsSyncCodeSurfaces,
  docsSyncSurfaceExcludes,
  docsSyncSurfaces,
} from "../docs-sync-config.ts";

function config(data: JsonObject): LoadedConfig {
  return {
    data,
    invalid: undefined,
    files: { user: "/u", project: undefined },
    host: "claude",
    warn: () => {},
  };
}

test.concurrent("no override: the built-in lists", () => {
  const c = config({});
  expect(docsSyncSurfaces(c)).toEqual([...DOCS_SYNC_DEFAULTS.surfaces]);
  expect(docsSyncSurfaceExcludes(c)).toEqual(["docs/releases/*", "*/docs/releases/*"]);
  expect(docsSyncCodeSurfaces(c)).toContain("*plugin.json");
});

test.concurrent("an override replaces the list; empty or non-array keeps the default", () => {
  const c = config({
    docsSync: { surfaces: ["only.md"], surfaceExcludes: [], codeSurfaces: "*.go" },
  });
  expect(docsSyncSurfaces(c)).toEqual(["only.md"]);
  expect(docsSyncSurfaceExcludes(c)).toEqual([...DOCS_SYNC_DEFAULTS.surfaceExcludes]);
  expect(docsSyncCodeSurfaces(c)).toEqual([...DOCS_SYNC_DEFAULTS.codeSurfaces]);
});

test.concurrent('members render like jq -r: non-strings as JSON, [""] means unset', () => {
  expect(docsSyncCodeSurfaces(config({ docsSync: { codeSurfaces: ["*.go", 7, null] } }))).toEqual([
    "*.go",
    "7",
    "null",
  ]);
  expect(docsSyncSurfaces(config({ docsSync: { surfaces: [""] } }))).toEqual([
    ...DOCS_SYNC_DEFAULTS.surfaces,
  ]);
  expect(docsSyncSurfaces(config({ docsSync: { surfaces: ["", "a.md"] } }))).toEqual(["", "a.md"]);
});
