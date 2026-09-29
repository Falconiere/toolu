import { expect, test } from "bun:test";
import type { JsonObject, LoadedConfig } from "../config-load.ts";
import {
  codexModel,
  configString,
  enabled,
  enabledExplicit,
  flagFalse,
  flagTrue,
  model,
} from "../config-read.ts";

function config(data: JsonObject): { config: LoadedConfig; warnings: string[] } {
  const warnings: string[] = [];
  const loaded: LoadedConfig = {
    data,
    invalid: undefined,
    files: { user: "/u/toolu.config.json", project: undefined },
    host: "claude",
    warn: (message) => warnings.push(message),
  };
  return { config: loaded, warnings };
}

test.concurrent('enabled defaults on and turns off for false or "false"', () => {
  const { config: c } = config({ hooks: { a: false, b: "false", c: true, d: 0 }, skills: [false] });
  expect([enabled(c, "hooks", "a"), enabled(c, "hooks", "b")]).toEqual([false, false]);
  expect([enabled(c, "hooks", "c"), enabled(c, "hooks", "d"), enabled(c, "hooks", "z")]).toEqual([
    true,
    true,
    true,
  ]);
  expect(enabled(c, "skills", "0")).toBe(true);
});

test.concurrent("flagTrue, flagFalse and enabledExplicit are default off", () => {
  const { config: c } = config({ p: { t: true, f: false, st: "true", sf: "false" } });
  expect([flagTrue(c, "p", "t"), flagTrue(c, "p", "st"), flagTrue(c, "p", "x")]).toEqual([
    true,
    false,
    false,
  ]);
  expect([flagFalse(c, "p", "f"), flagFalse(c, "p", "sf"), flagFalse(c, "p", "x")]).toEqual([
    true,
    false,
    false,
  ]);
  expect([enabledExplicit(c, "p", "t"), enabledExplicit(c, "p", "st")]).toEqual([true, true]);
  expect(enabledExplicit(c, "p", "sf")).toBe(false);
});

test.concurrent("configString: absent is silent, unqualified warns and falls back", () => {
  const { config: c, warnings } = config({
    a: { ok: "block", bad: "maybe", num: 3 },
    s: "flat",
  });
  const modes = ["block", "advise", "off"] as const;
  expect(configString(c, "a.ok", "advise", modes)).toBe("block");
  expect(configString(c, "a.missing", "advise", modes)).toBe("advise");
  expect(configString(c, "s.deeper", "advise", modes)).toBe("advise");
  expect(warnings).toEqual([]);
  expect(configString(c, "a.bad", "advise", modes)).toBe("advise");
  expect(configString(c, "a.num", "advise", modes)).toBe("advise");
  expect(warnings).toEqual([
    "a.bad: 'maybe' is not an allowed value (block advise off); using advise",
    "a.num: value is not a string; using advise",
  ]);
});

test.concurrent("model: aliases pass, junk warns, false and empty are unset", () => {
  const { config: c, warnings } = config({
    models: {
      review: "opus",
      synthesis: "gpt",
      mechanical: false,
      exploration: "",
      architecture: 3,
    },
  });
  expect(model(c, "review")).toBe("opus");
  expect(model(c, "mechanical")).toBe("haiku");
  expect(model(c, "exploration")).toBe("sonnet");
  expect(model(c, "synthesis")).toBe("opus");
  expect(model(c, "architecture")).toBe("opus");
  expect(warnings).toEqual([
    "models.synthesis: 'gpt' is not a model alias (haiku sonnet opus fable inherit); using opus",
    "models.architecture: '3' is not a model alias (haiku sonnet opus fable inherit); using opus",
  ]);
  expect(() => model(c, "reviewer")).toThrow("unknown model class 'reviewer'");
});

test.concurrent("codexModel: model and effort fall back independently", () => {
  const { config: c, warnings } = config({
    models: {
      codex: {
        review: { model: "", reasoningEffort: "extreme" },
        synthesis: { model: "gpt-x", reasoningEffort: 3 },
        architecture: { model: "gpt-arch", reasoningEffort: "max" },
        // jq `// null` reads false as unset: bash falls back without a warning.
        exploration: { model: false, reasoningEffort: false },
      },
    },
  });
  expect(codexModel(c, "architecture")).toEqual({ model: "gpt-arch", reasoningEffort: "max" });
  expect(codexModel(c, "mechanical")).toEqual({ model: "gpt-5.6-luna", reasoningEffort: "medium" });
  expect(codexModel(c, "exploration")).toEqual({
    model: "gpt-5.6-terra",
    reasoningEffort: "medium",
  });
  expect(codexModel(c, "review")).toEqual({ model: "gpt-5.6-terra", reasoningEffort: "high" });
  expect(codexModel(c, "synthesis")).toEqual({ model: "gpt-x", reasoningEffort: "high" });
  expect(warnings).toEqual([
    "models.codex.review.model: value is not a non-empty string; using gpt-5.6-terra",
    "models.codex.review.reasoningEffort: 'extreme' is not supported (low medium high xhigh max ultra); using high",
    "models.codex.synthesis.reasoningEffort: value is not a string; using high",
  ]);
});
