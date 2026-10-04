import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { lstatSync, mkdirSync, symlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import { TARGET, enabledOf, pluginsOf, profile, selection, toolu } from "./cli-fixture";

const CLI = resolve(import.meta.dir, "../../cli.ts");

describe("scopes", () => {
  test("--scope project writes the worktree root config and project selection", () => {
    using p = profile();
    mkdirSync(p.local("sub"));
    const run = spawnSync(
      process.execPath,
      [CLI, "install", "jev", "--host", "opencode", "--scope", "project"],
      {
        cwd: p.local("sub"),
        env: p.env,
        encoding: "utf8",
      },
    );
    expect(run.status).toBe(0);
    expect(pluginsOf(p.read(p.local("opencode.json")))).toEqual([TARGET]);
    expect(enabledOf(p.read(p.local(".opencode/toolu/plugins.json")))).toEqual(["jev"]);
    expect(p.exists(p.global("opencode.json"))).toBe(false);
  });

  test("--scope local is a usage error", () => {
    using p = profile();
    expect(toolu(p, ["install", "--scope", "local"]).code).toBe(2);
  });

  test("the package in both scopes needs --scope for remove and update; nothing is written", () => {
    using p = profile();
    const both = `{ "plugin": ["@toolu/opencode@1.0.0"] }\n`;
    p.write(p.global("opencode.json"), both);
    p.write(p.local("opencode.json"), both);
    for (const args of [["remove", "--yes", "toolu"], ["update"]]) {
      const run = toolu(p, args);
      expect(run.code).toBe(2);
      expect(run.stderr).toContain("choose one with --scope");
    }
    expect(p.read(p.global("opencode.json"))).toBe(both);
    expect(p.read(p.local("opencode.json"))).toBe(both);
    expect(toolu(p, ["update", "--scope", "project"]).code).toBe(0);
    expect(pluginsOf(p.read(p.local("opencode.json")))).toEqual([TARGET]);
    expect(p.read(p.global("opencode.json"))).toBe(both);
  });
});

describe("failures write nothing", () => {
  test("malformed JSONC, a non-array plugin and an invalid selection exit 1 naming the file", () => {
    using p = profile();
    p.write(p.global("opencode.jsonc"), '{ "plugin": [ ');
    const bad = toolu(p, ["install"]);
    expect(bad.code).toBe(1);
    expect(bad.stderr).toContain(p.global("opencode.jsonc"));
    p.write(p.global("opencode.jsonc"), '{ "plugin": "x" }');
    expect(toolu(p, ["install"]).stderr).toContain('"plugin" must be an array');
    p.write(p.global("opencode.jsonc"), "{}");
    p.write(p.global("toolu/plugins.json"), "{ nope");
    const invalid = toolu(p, ["install", "jev"]);
    expect(invalid.code).toBe(1);
    expect(invalid.stderr).toContain(p.global("toolu/plugins.json"));
    expect(p.read(p.global("opencode.jsonc"))).toBe("{}");
    expect(p.read(p.global("toolu/plugins.json"))).toBe("{ nope");
  });

  test("remove without --yes exits 3", () => {
    using p = profile();
    const config = `{ "plugin": ["${TARGET}"] }\n`;
    p.write(p.global("opencode.json"), config);
    p.write(p.global("toolu/plugins.json"), selection(["jev"]));
    for (const name of ["jev", "toolu"]) {
      const run = toolu(p, ["remove", name]);
      expect(run.code).toBe(3);
      expect(run.stderr).toContain("remove requires --yes");
    }
    expect(p.read(p.global("opencode.json"))).toBe(config);
    expect(p.read(p.global("toolu/plugins.json"))).toBe(selection(["jev"]));
  });

  test("an invalid TOOLU_OPENCODE_PACKAGE is a usage error", () => {
    using p = profile();
    expect(toolu(p, ["install"], { TOOLU_OPENCODE_PACKAGE: "evil@1.0.0" }).code).toBe(2);
    expect(p.exists(p.global("opencode.json"))).toBe(false);
  });

  test("a symlinked config is edited through the link", () => {
    using p = profile();
    p.write(p.local("dotfiles/opencode.json"), "{}\n");
    mkdirSync(p.global(""), { recursive: true });
    symlinkSync(p.local("dotfiles/opencode.json"), p.global("opencode.json"));
    expect(toolu(p, ["install"]).code).toBe(0);
    expect(lstatSync(p.global("opencode.json")).isSymbolicLink()).toBe(true);
    expect(pluginsOf(p.read(p.local("dotfiles/opencode.json")))).toEqual([TARGET]);
  });
});

describe("host detection", () => {
  test("with only opencode on PATH, a host-less run manages OpenCode", () => {
    using p = profile();
    const bin = p.local(".bin");
    mkdirSync(bin);
    symlinkSync("/usr/bin/true", join(bin, "opencode"));
    const git = Bun.which("git");
    if (git === null) throw new Error("git is required");
    symlinkSync(git, join(bin, "git"));
    p.write(p.global("toolu/plugins.json"), selection(["jev"]));
    const run = spawnSync(process.execPath, [CLI, "list"], {
      cwd: p.project,
      env: { ...p.env, PATH: bin },
      encoding: "utf8",
    });
    expect(run.status).toBe(0);
    expect(run.stdout).toContain("@toolu/opencode: not configured");
  });
});
