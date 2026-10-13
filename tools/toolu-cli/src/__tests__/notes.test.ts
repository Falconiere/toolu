import { describe, expect, test } from "bun:test";
import {
  TARGET,
  enabledNames,
  enabledOf,
  listed,
  pluginsOf,
  profile,
  selection,
  toolu,
} from "./cli-fixture";

describe("which selection an edit lands in", () => {
  test("without --scope, install enables in the project selection when one governs", () => {
    using p = profile();
    expect(toolu(p, ["install", "ast-grep", "--scope", "project"]).code).toBe(0);
    const run = toolu(p, ["install", "jev"]);
    expect(run.code).toBe(0);
    expect(run.stdout).toContain(`enabled in ${p.local(".opencode/toolu/plugins.json")}`);
    expect(enabledOf(p.read(p.local(".opencode/toolu/plugins.json")))).toEqual(["ast-grep", "jev"]);
    expect(p.exists(p.global("toolu/plugins.json"))).toBe(false);
    expect(enabledNames(p)).toEqual(["ast-grep", "jev"]);
  });

  test("without --scope, a leaf remove disables in the project selection when one governs", () => {
    using p = profile();
    p.write(p.local(".opencode/toolu/plugins.json"), selection(["ast-grep", "jev"]));
    p.write(p.global("opencode.json"), `{ "plugin": ["${TARGET}"] }\n`);
    const run = toolu(p, ["remove", "--yes", "jev"]);
    expect(run.code).toBe(0);
    expect(run.stdout).toContain(`disabled in ${p.local(".opencode/toolu/plugins.json")}`);
    expect(enabledOf(p.read(p.local(".opencode/toolu/plugins.json")))).toEqual(["ast-grep"]);
    expect(p.exists(p.global("toolu/plugins.json"))).toBe(false);
  });

  test("an explicit --scope user edit that a project selection overrides says so", () => {
    using p = profile();
    p.write(p.local(".opencode/toolu/plugins.json"), selection(["jev"]));
    p.write(p.global("opencode.json"), `{ "plugin": ["${TARGET}"] }\n`);
    p.write(p.global("toolu/plugins.json"), selection(["jev"]));
    const note = `no effect in this project: ${p.local(".opencode/toolu/plugins.json")} governs it`;
    const install = toolu(p, ["install", "ast-grep", "--scope", "user"]);
    expect(install.code).toBe(0);
    expect(install.stdout).toContain(note);
    expect(enabledOf(p.read(p.global("toolu/plugins.json")))).toEqual(["jev", "ast-grep"]);
    const remove = toolu(p, ["remove", "--yes", "jev", "--scope", "user"]);
    expect(remove.code).toBe(0);
    expect(remove.stdout).toContain(note);
    expect(enabledNames(p)).toEqual(["jev"]);
  });

  test("a named install over a shadowed global entry is a fresh install of the closure", () => {
    using p = profile();
    p.write(p.global("config.json"), `{ "plugin": ["${TARGET}"] }\n`);
    p.write(p.global("opencode.json"), `{ "plugin": ["other-plugin"] }\n`);
    expect(toolu(p, ["install", "jev"]).code).toBe(0);
    expect(enabledOf(p.read(p.global("toolu/plugins.json")))).toEqual(["jev"]);
    expect(enabledNames(p)).toEqual(["jev"]);
  });
});

describe("notes on config layering", () => {
  test("an entry in a shadowed global file does not count; install adds it where the host loads", () => {
    using p = profile();
    const shadowed = `{ "plugin": ["${TARGET}"] }\n`;
    p.write(p.global("config.json"), shadowed);
    p.write(p.global("opencode.json"), `{ "plugin": ["other-plugin"] }\n`);
    const run = toolu(p, ["install"]);
    expect(run.code).toBe(0);
    expect(run.stdout).toContain(`added ${TARGET} to ${p.global("opencode.json")}`);
    expect(pluginsOf(p.read(p.global("opencode.json")))).toEqual(["other-plugin", TARGET]);
    expect(p.read(p.global("config.json"))).toBe(shadowed);
    expect(listed(p).get("toolu")?.installed).toBe(true);
  });

  test("a global install shadowed by a project's empty plugin list is skew, exit 0", () => {
    using p = profile();
    p.write(p.local("opencode.json"), `{ "plugin": [] }\n`);
    const run = toolu(p, ["install"]);
    expect(run.code).toBe(0);
    expect(run.stdout).toContain(
      `no effect here: ${p.local("opencode.json")} sets an empty plugin list`,
    );
  });

  test("filling a project's empty plugin list says global plugins load again", () => {
    using p = profile();
    p.write(p.global("opencode.json"), `{ "plugin": ["other-global-plugin"] }\n`);
    p.write(p.local("opencode.json"), `{ "plugin": [] }\n`);
    const run = toolu(p, ["install", "--scope", "project"]);
    expect(run.code).toBe(0);
    expect(run.stdout).toContain(
      `${p.local("opencode.json")} no longer empties the plugin list, so plugins from earlier config files load here again`,
    );
    expect(pluginsOf(p.read(p.local("opencode.json")))).toEqual([TARGET]);
  });

  test("removing the last entry of a global file names the lower file that applies again", () => {
    using p = profile();
    p.write(p.global("config.json"), `{ "plugin": ["older-plugin"] }\n`);
    p.write(p.global("opencode.json"), `{ "plugin": ["${TARGET}"] }\n`);
    const run = toolu(p, ["remove", "--yes", "toolu"]);
    expect(run.code).toBe(0);
    expect(run.stdout).toContain(`${p.global("config.json")}'s plugin list applies again`);
    expect(JSON.parse(p.read(p.global("opencode.json")))).toEqual({});
  });
});
