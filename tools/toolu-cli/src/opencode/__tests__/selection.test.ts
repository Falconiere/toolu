import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";
import { readMarketplace } from "../../catalog/manifest";
import { CliError, EXIT } from "../../exit";
import {
  dependentsBlocking,
  effectiveEnabled,
  readSelectionFile,
  skillsDisabled,
} from "../selection";
import { REPO_ROOT, profile, selection } from "./cli-fixture";

const marketplace = await readMarketplace(resolve(REPO_ROOT, ".claude-plugin/marketplace.json"));

describe("selection files", () => {
  test("absent is undefined; invalid JSON or schema fails closed naming the file", async () => {
    using p = profile();
    expect(await readSelectionFile(p.local("none.json"))).toBeUndefined();
    p.write(p.local("bad.json"), "{ not json");
    p.write(p.local("extra.json"), JSON.stringify({ version: 1, enabled: [], host: "x" }));
    const names = ["bad.json", "extra.json"];
    const errors = await Promise.all(
      names.map((name) => readSelectionFile(p.local(name)).catch((caught: unknown) => caught)),
    );
    for (const [i, error] of errors.entries()) {
      if (!(error instanceof CliError)) throw new Error(`expected a CliError for ${names[i]}`);
      expect(error.code).toBe(EXIT.failed);
      expect(error.message).toContain(p.local(names[i] ?? ""));
    }
    p.write(p.local("ok.json"), selection(["jev"]));
    expect(await readSelectionFile(p.local("ok.json"))).toEqual(["jev"]);
  });
});

describe("effective enabled set", () => {
  test("closure over the real catalog adds every dependency", () => {
    const enabled = effectiveEnabled(marketplace, ["delivery-flow"], new Set());
    expect([...enabled].toSorted()).toEqual(
      ["brainstorm", "delivery-flow", "pr-babysit", "toolu", "toolu-review"].toSorted(),
    );
  });

  test("no list means the whole catalog; unknown names are ignored", () => {
    expect(effectiveEnabled(marketplace, undefined, new Set()).size).toBe(
      marketplace.plugins.length,
    );
    expect([...effectiveEnabled(marketplace, ["jev", "not-a-plugin"], new Set())]).toEqual(["jev"]);
  });

  test("skills.<name>: false drops a listed plugin, but closure re-adds a needed dependency", async () => {
    using p = profile();
    p.write(
      p.local(".opencode/toolu.config.json"),
      JSON.stringify({ version: 1, skills: { jev: false, "pr-babysit": false } }),
    );
    const disabled = await skillsDisabled(p.local(".opencode/toolu.config.json"));
    const enabled = effectiveEnabled(marketplace, ["jev", "pr-babysit", "delivery-flow"], disabled);
    expect(enabled.has("jev")).toBe(false);
    expect(enabled.has("pr-babysit")).toBe(true);
  });

  test("dependents block a removal unless they are removed too", () => {
    const enabled = effectiveEnabled(marketplace, ["delivery-flow"], new Set());
    expect(dependentsBlocking(marketplace, enabled, new Set(["pr-babysit"]), "pr-babysit")).toEqual(
      ["delivery-flow"],
    );
    expect(
      dependentsBlocking(
        marketplace,
        enabled,
        new Set(["pr-babysit", "delivery-flow"]),
        "pr-babysit",
      ),
    ).toEqual([]);
  });
});
