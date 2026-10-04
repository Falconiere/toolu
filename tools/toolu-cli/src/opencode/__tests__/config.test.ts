import { describe, expect, test } from "bun:test";
import { lstatSync, readFileSync, symlinkSync } from "node:fs";
import { EXIT } from "../../exit";
import { effectiveEntry, packageName, pluginArray, targetSpec, versionOf } from "../entries";
import { editJsonc, parseConfig, readConfigFile, writeAtomic, type ConfigFile } from "../jsonc";
import { profile } from "./cli-fixture";

const COMMENTED = `// user settings
{
  /* keep me */
  "plugin": [
    "file:///opt/probe.ts", // unrelated
  ],
  "share": "disabled",
}
`;

function file(path: string, text: string | undefined): ConfigFile {
  return { path, text, data: text === undefined ? undefined : parseConfig(path, text) };
}

describe("plugin specs", () => {
  test("package names of scoped, unscoped, pinned, file and URL specs", () => {
    expect(packageName("@toolu/opencode@7.8.0")).toBe("@toolu/opencode");
    expect(packageName("@toolu/opencode")).toBe("@toolu/opencode");
    expect(packageName("@toolu/opencode@file:/tmp/a.tgz")).toBe("@toolu/opencode");
    expect(packageName("other@1.0.0")).toBe("other");
    expect(packageName("file:///x/toolu.ts")).not.toBe("@toolu/opencode");
    expect(versionOf("@toolu/opencode")).toBe("latest");
    expect(versionOf("@toolu/opencode@^7.0.0")).toBe("^7.0.0");
  });

  test("the target spec defaults to the CLI version and an override must name the package", () => {
    expect(targetSpec({}, "7.8.0")).toBe("@toolu/opencode@7.8.0");
    expect(targetSpec({ TOOLU_OPENCODE_PACKAGE: "@toolu/opencode@file:/t.tgz" }, "7.8.0")).toBe(
      "@toolu/opencode@file:/t.tgz",
    );
    expect(() => targetSpec({ TOOLU_OPENCODE_PACKAGE: "evil@1.0.0" }, "7.8.0")).toThrow(
      expect.objectContaining({ code: EXIT.usage }),
    );
  });
});

describe("JSONC edits keep the user's file", () => {
  test("appending, replacing a tuple spec and removing leave comments and other entries", () => {
    const added = editJsonc(COMMENTED, ["plugin", 1], "@toolu/opencode@1.0.0", true);
    expect(added).toContain("// user settings");
    expect(added).toContain("/* keep me */");
    expect(added).toContain('"file:///opt/probe.ts", // unrelated');
    expect(pluginArray(file("a", added))).toEqual([
      "file:///opt/probe.ts",
      "@toolu/opencode@1.0.0",
    ]);
    const removed = editJsonc(added, ["plugin", 1], undefined);
    expect(pluginArray(file("a", removed))).toEqual(["file:///opt/probe.ts"]);
    expect(removed).toContain("/* keep me */");
    const tuple = '{ "plugin": [["@toolu/opencode@1.0.0", { "x": 1 }]] }\n';
    const swapped = editJsonc(tuple, ["plugin", 0, 0], "@toolu/opencode@2.0.0");
    expect(pluginArray(file("t", swapped))).toEqual([["@toolu/opencode@2.0.0", { x: 1 }]]);
  });
});

describe("effective entry mirrors the pinned host's merge", () => {
  const g = (name: string, plugins?: unknown[]) =>
    file(`/g/${name}`, plugins === undefined ? undefined : JSON.stringify({ plugin: plugins }));
  const p = (name: string, plugins?: unknown[]) =>
    file(`/p/${name}`, plugins === undefined ? undefined : JSON.stringify({ plugin: plugins }));

  test("R1: the highest-priority global file defining plugin replaces the others", () => {
    const global = [
      g("config.json", ["@toolu/opencode@1.0.0"]),
      g("opencode.json", ["x"]),
      g("opencode.jsonc"),
    ];
    expect(effectiveEntry(global, [])).toBeUndefined();
  });

  test("R2: project entries concatenate and the last one for the package wins", () => {
    const global = [
      g("config.json"),
      g("opencode.json", ["@toolu/opencode@1.0.0", "x"]),
      g("opencode.jsonc"),
    ];
    const project = [p("opencode.json", [["@toolu/opencode@2.0.0", { a: 1 }]])];
    expect(effectiveEntry(global, project)).toMatchObject({
      spec: "@toolu/opencode@2.0.0",
      tuple: true,
    });
  });

  test("R3: an empty project array drops the global entry", () => {
    const global = [
      g("config.json"),
      g("opencode.json", ["@toolu/opencode@1.0.0"]),
      g("opencode.jsonc"),
    ];
    expect(effectiveEntry(global, [p("opencode.json", [])])).toBeUndefined();
  });
});

describe("reading and writing config files", () => {
  test("absent, malformed and non-array files", async () => {
    using prof = profile();
    expect((await readConfigFile(prof.local("none.json"))).text).toBeUndefined();
    prof.write(prof.local("bad.jsonc"), '{ "plugin": [ "a" ');
    expect(readConfigFile(prof.local("bad.jsonc"))).rejects.toMatchObject({
      code: EXIT.failed,
      message: expect.stringContaining("bad.jsonc is not valid JSONC"),
    });
    prof.write(prof.local("str.json"), '{ "plugin": "a" }');
    const str = await readConfigFile(prof.local("str.json"));
    expect(() => pluginArray(str)).toThrow(/"plugin" must be an array/);
  });

  test("a write through a symlink edits the target and keeps the link", async () => {
    using prof = profile();
    prof.write(prof.local("real.json"), "{}\n");
    symlinkSync(prof.local("real.json"), prof.local("link.json"));
    await writeAtomic(prof.local("link.json"), '{ "plugin": [] }\n');
    expect(lstatSync(prof.local("link.json")).isSymbolicLink()).toBe(true);
    expect(readFileSync(prof.local("real.json"), "utf8")).toBe('{ "plugin": [] }\n');
  });
});
