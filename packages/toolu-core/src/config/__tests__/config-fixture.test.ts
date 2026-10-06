/**
 * The shared config fixture (#414): the TypeScript loader and resolvers
 * reproduce every case of `fixtures/config/expected.json`. The Rust loader
 * (`crates/core/runtime/tests/config_fixture.rs`) checks the same cases.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import {
  codexModel,
  docsSyncCodeSurfaces,
  docsSyncSurfaceExcludes,
  docsSyncSurfaces,
  enabled,
  enabledExplicit,
  flagFalse,
  flagTrue,
  gateMode,
  gatePreset,
  loadConfig,
  model,
  qualityFlag,
  qualityThreshold,
  tsMaxFileLinesResolved,
  type LoadedConfig,
  type QualityLang,
} from "../config.ts";

const FIXTURES = resolve(import.meta.dir, "../../../../../fixtures/config");
const PlacementSchema = z
  .union([z.strictObject({ file: z.string() }), z.strictObject({ text: z.string() })])
  .nullable();
const CaseSchema = z.strictObject({
  name: z.string().min(1),
  host: z.enum(["claude", "codex"]),
  user: PlacementSchema,
  project: PlacementSchema,
  expect: z.strictObject({
    invalid: z.string().nullable(),
    warnings: z.array(z.string()),
    data: z.record(z.string(), z.unknown()),
    resolved: z.record(
      z.string(),
      z.strictObject({ value: z.unknown(), warnings: z.array(z.string()).optional() }),
    ),
  }),
});
const LangSchema = z.enum(["ts", "rust", "python"]);
const cases = readCaseFile(join(FIXTURES, "expected.json")).map((raw) => CaseSchema.parse(raw));

function placementText(placement: z.infer<typeof PlacementSchema>): string | undefined {
  if (placement === null) return undefined;
  return "file" in placement
    ? readFileSync(join(FIXTURES, placement.file), "utf8")
    : placement.text;
}

function lang(name: string | undefined): QualityLang {
  return LangSchema.parse(name);
}

function threshold(config: LoadedConfig, which: QualityLang, key: string, root: string): number {
  const common = z.enum(["maxFileLines", "maxFnLines"]);
  if (which === "rust") {
    return qualityThreshold(config, "rust", common.or(z.literal("maxImplLines")).parse(key), {
      root,
    });
  }
  return qualityThreshold(config, which, common.parse(key), { root });
}

/** The value of the call `id` names, e.g. `gateMode pushReview codex`. */
function resolveCall(config: LoadedConfig, id: string, root: string): unknown {
  const [fn = "", a = "", b = ""] = id.split(" ");
  switch (fn) {
    case "gatePreset":
      return gatePreset(config);
    case "gateMode":
      return gateMode(config, a, { host: z.enum(["claude", "codex"]).parse(b) });
    case "model":
      return model(config, a);
    case "codexModel":
      return codexModel(config, a);
    case "qualityThreshold":
      return threshold(config, lang(a), b, root);
    case "tsMaxFileLinesResolved":
      return tsMaxFileLinesResolved(config, { root });
    case "qualityFlag":
      return qualityFlag(config, lang(a), b, true);
    case "docsSync":
      return {
        surfaces: docsSyncSurfaces,
        surfaceExcludes: docsSyncSurfaceExcludes,
        codeSurfaces: docsSyncCodeSurfaces,
      }[z.enum(["surfaces", "surfaceExcludes", "codeSurfaces"]).parse(a)](config);
    case "enabled":
      return enabled(config, a, b);
    case "flagTrue":
      return flagTrue(config, a, b);
    case "flagFalse":
      return flagFalse(config, a, b);
    case "enabledExplicit":
      return enabledExplicit(config, a, b);
    default:
      throw new Error(`unknown call ${id}`);
  }
}

test("the fixture covers every config file and every rejection", () => {
  expect(cases.length).toBe(35);
  expect(cases.filter((c) => c.expect.invalid !== null).length).toBe(12);
  expect(cases.every((c) => Object.keys(c.expect.resolved).length === 81)).toBe(true);
});

for (const c of cases) {
  test.concurrent(c.name, () => {
    using sb = createSandbox();
    const paths = {
      user: join(sb.configDir(c.host, "user"), "toolu.config.json"),
      project: join(sb.configDir(c.host, "project"), "toolu.config.json"),
    };
    for (const scope of ["user", "project"] as const) {
      const text = placementText(c[scope]);
      if (text === undefined) continue;
      mkdirSync(join(paths[scope], ".."), { recursive: true });
      writeFileSync(paths[scope], text);
    }
    const tokens = (text: string) =>
      text.replaceAll(paths.user, "$USER_CONFIG").replaceAll(paths.project, "$PROJECT_CONFIG");
    const warnings: string[] = [];
    const env = { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project };
    const config = loadConfig({ env, host: c.host, warn: (m) => warnings.push(m) });
    expect(config.invalid === undefined ? null : tokens(config.invalid)).toBe(c.expect.invalid);
    expect(warnings.splice(0).map(tokens)).toEqual(c.expect.warnings);
    expect(config.data).toEqual(c.expect.data);
    for (const [id, want] of Object.entries(c.expect.resolved)) {
      expect({
        id,
        value: resolveCall(config, id, sb.project),
        warnings: warnings.splice(0),
      }).toEqual({
        id,
        value: want.value,
        warnings: want.warnings ?? [],
      });
    }
  });
}
