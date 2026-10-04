import { describe, expect, test } from "bun:test";
import { readdirSync } from "node:fs";
import { newConfigText } from "../jsonc";
import { TARGET, enabledNames, enabledOf, listed, profile, selection, toolu } from "./cli-fixture";

const COMMENTED = `// my OpenCode settings
{
  /* providers live elsewhere */
  "plugin": [
    "file:///opt/probe.ts", // unrelated plugin
  ],
}
`;

function keepsUserText(text: string): boolean {
  return (
    text.includes("// my OpenCode settings") &&
    text.includes("/* providers live elsewhere */") &&
    text.includes('"file:///opt/probe.ts", // unrelated plugin')
  );
}

describe("install", () => {
  test("a bare install on a clean profile adds one entry and no selection; a rerun changes nothing", () => {
    using p = profile();
    const first = toolu(p, ["install"]);
    expect(first.code).toBe(0);
    expect(p.read(p.global("opencode.json"))).toBe(newConfigText(TARGET));
    expect(p.exists(p.global("toolu/plugins.json"))).toBe(false);
    const again = toolu(p, ["install"]);
    expect(again.code).toBe(0);
    expect(again.stdout).toContain("already configured");
    expect(p.read(p.global("opencode.json"))).toBe(newConfigText(TARGET));
    expect(enabledNames(p).length).toBe(listed(p).size);
  });

  test("a named install on a clean profile enables exactly its closure", () => {
    using p = profile();
    expect(toolu(p, ["install", "jev"]).code).toBe(0);
    expect(enabledOf(p.read(p.global("toolu/plugins.json")))).toEqual(["jev"]);
    expect(enabledNames(p)).toEqual(["jev"]);
    expect(toolu(p, ["install", "pr-babysit"]).code).toBe(0);
    expect(enabledNames(p)).toEqual(["jev", "pr-babysit", "toolu"]);
  });

  test("an existing pin is kept and reported as skew", () => {
    using p = profile();
    const pinned = '{ "plugin": ["@toolu/opencode@1.0.0"] }\n';
    p.write(p.global("opencode.json"), pinned);
    const run = toolu(p, ["install"]);
    expect(run.code).toBe(0);
    expect(run.stdout).toContain("@toolu/opencode@1.0.0");
    expect(run.stdout).toContain(`this CLI installs ${TARGET}`);
    expect(run.stdout).toContain("update --host opencode");
    expect(p.read(p.global("opencode.json"))).toBe(pinned);
  });
});

describe("the full lifecycle keeps the user's JSONC", () => {
  test("install, enable, disable, update and remove toolu", () => {
    using p = profile();
    const path = p.global("opencode.jsonc");
    p.write(path, COMMENTED);
    expect(
      toolu(p, ["install", "jev"], { TOOLU_OPENCODE_PACKAGE: "@toolu/opencode@1.0.0" }).code,
    ).toBe(0);
    expect(keepsUserText(p.read(path))).toBe(true);
    expect(p.read(path)).toContain('"@toolu/opencode@1.0.0"');
    expect(p.exists(p.global("opencode.json"))).toBe(false);
    expect(toolu(p, ["install", "context7"]).code).toBe(0);
    expect(enabledNames(p)).toEqual(["context7", "jev"]);
    expect(toolu(p, ["remove", "--yes", "context7"]).code).toBe(0);
    expect(enabledNames(p)).toEqual(["jev"]);
    const update = toolu(p, ["update"]);
    expect(update.code).toBe(0);
    expect(update.stdout).toContain(`@toolu/opencode@1.0.0 -> ${TARGET}`);
    expect(listed(p).get("jev")?.version).toBe(TARGET.split("@").at(-1));
    expect(toolu(p, ["update"]).stdout).toContain("current at");
    const removed = toolu(p, ["remove", "--yes", "toolu"]);
    expect(removed.code).toBe(0);
    expect(removed.stdout).toContain("selection kept at");
    expect(keepsUserText(p.read(path))).toBe(true);
    expect(p.read(path)).not.toContain("@toolu/opencode");
    expect(listed(p).get("toolu")?.installed).toBe(false);
  });
});

describe("update", () => {
  test("a tuple keeps its options; no entry means nothing to update", () => {
    using p = profile();
    p.write(
      p.local("opencode.json"),
      '{\n  "plugin": [["@toolu/opencode@1.0.0", { "x": 1 }]]\n}\n',
    );
    expect(toolu(p, ["update"]).code).toBe(0);
    expect(JSON.parse(p.read(p.local("opencode.json")))).toEqual({
      plugin: [[TARGET, { x: 1 }]],
    });
    using empty = profile();
    const none = toolu(empty, ["update"]);
    expect(none.code).toBe(1);
    expect(none.stderr).toContain("nothing to update");
  });
});

describe("remove", () => {
  test("a needed dependency is refused, absent names are no-ops", () => {
    using p = profile();
    p.write(p.global("opencode.json"), `{ "plugin": ["${TARGET}"] }\n`);
    p.write(p.global("toolu/plugins.json"), selection(["delivery-flow"]));
    const refused = toolu(p, ["remove", "--yes", "pr-babysit"]);
    expect(refused.code).toBe(1);
    expect(refused.stdout).toContain("required by delivery-flow");
    expect(p.read(p.global("toolu/plugins.json"))).toBe(selection(["delivery-flow"]));
    const absent = toolu(p, ["remove", "--yes", "jev"]);
    expect(absent.code).toBe(0);
    expect(absent.stdout).toContain("not enabled");
    using empty = profile();
    expect(toolu(empty, ["remove", "--yes", "toolu"]).code).toBe(0);
  });

  test("removing the last entry deletes the plugin key so a reset never drops other plugins", () => {
    using p = profile();
    p.write(p.local("opencode.json"), `{\n  "plugin": ["${TARGET}"],\n  "share": "disabled"\n}\n`);
    expect(toolu(p, ["remove", "--yes", "toolu"]).code).toBe(0);
    expect(JSON.parse(p.read(p.local("opencode.json")))).toEqual({ share: "disabled" });
  });
});

describe("list reflects the files as they are", () => {
  test("project pin and selection win; skills false switches a plugin off", () => {
    using p = profile();
    p.write(p.global("opencode.json"), '{ "plugin": ["@toolu/opencode@1.0.0"] }\n');
    p.write(p.local("opencode.json"), '{ "plugin": ["@toolu/opencode@2.0.0"] }\n');
    p.write(p.global("toolu/plugins.json"), selection(["context7"]));
    p.write(p.local(".opencode/toolu/plugins.json"), selection(["jev", "exa-search"]));
    p.write(
      p.local(".opencode/toolu.config.json"),
      JSON.stringify({ version: 1, skills: { "exa-search": false } }),
    );
    expect(listed(p).get("jev")?.version).toBe("2.0.0");
    expect(enabledNames(p)).toEqual(["jev"]);
    expect(toolu(p, ["list"]).stdout).toContain(".opencode/toolu/plugins.json");
  });
});

describe("dry run", () => {
  test("install, update and remove print planned changes and write nothing", () => {
    using p = profile();
    const install = toolu(p, ["install", "jev", "--dry-run"]);
    expect(install.code).toBe(0);
    expect(install.stdout).toContain("Planned changes (nothing was written):");
    expect(install.stdout).toContain(`add ${TARGET} to ${p.global("opencode.json")}`);
    expect(p.exists(p.home + "/.config")).toBe(false);
    p.write(p.global("opencode.json"), '{ "plugin": ["@toolu/opencode@1.0.0"] }\n');
    expect(toolu(p, ["update", "--dry-run"]).stdout).toContain(
      `update @toolu/opencode@1.0.0 -> ${TARGET}`,
    );
    expect(toolu(p, ["remove", "--yes", "toolu", "--dry-run"]).stdout).toContain(
      "remove @toolu/opencode from",
    );
    expect(p.read(p.global("opencode.json"))).toBe('{ "plugin": ["@toolu/opencode@1.0.0"] }\n');
    expect(readdirSync(p.global(""))).toEqual(["opencode.json"]);
  });
});
