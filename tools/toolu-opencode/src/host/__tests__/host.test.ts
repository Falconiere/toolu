import { expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { detectHost } from "../detect.ts";
import { opencodeConfigRoot, opencodeDataRoot, opencodePluginSelectionPath } from "../roots.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

test("detectHost honors TOOLU_HOST_OVERRIDE=opencode", () => {
  expect(detectHost({ env: { TOOLU_HOST_OVERRIDE: "opencode" } })).toBe("opencode");
});

test("detectHost uses OPENCODE_* env and .opencode project marker", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-oc-host-"));
  mkdirSync(join(project, ".opencode"), { recursive: true });
  expect(detectHost({ env: { OPENCODE_HOME: "/tmp/oc" } })).toBe("opencode");
  expect(detectHost({ env: {}, projectRoot: project })).toBe("opencode");
});

test("opencode roots honor TOOLU_CONFIG_DIR and TOOLU_OPENCODE_HOME", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-oc-roots-"));
  const explicit = join(project, "data");
  expect(opencodeDataRoot({ dataRoot: explicit })).toBe(explicit);
  expect(opencodeConfigRoot({ env: { TOOLU_CONFIG_DIR: "/cfg" } })).toBe("/cfg");
  expect(opencodeConfigRoot({ env: { TOOLU_OPENCODE_HOME: "/oc-home" } })).toBe("/oc-home");
  const derived = opencodeDataRoot({ projectRoot: project });
  expect(derived).toBe(join(project, ".opencode", "toolu", "state"));
});

test("selection path lives under project .opencode", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-oc-sel-"));
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(opencodePluginSelectionPath(project), '{"version":1,"enabled":["toolu"]}\n');
  expect(opencodePluginSelectionPath(project)).toContain(".opencode/toolu/plugins.json");
});
