import { describe, expect, test } from "bun:test";
import { join } from "node:path";
import { opencodeConfigRoot } from "@toolu/opencode/host";
import { selectPluginsWithDependencies } from "@toolu/opencode/select";
import { REPO_ROOT, enabledNames, profile, toolu } from "./cli-fixture";

describe("the adapter accepts what the CLI writes", () => {
  test("a CLI-written selection resolves to the set list reports", () => {
    using p = profile();
    expect(toolu(p, ["install", "delivery-flow", "jev"]).code).toBe(0);
    expect(toolu(p, ["remove", "--yes", "jev"]).code).toBe(0);
    expect(toolu(p, ["install", "ast-grep", "--scope", "project"]).code).toBe(0);
    const selected = selectPluginsWithDependencies(
      join(REPO_ROOT, "plugins"),
      p.project,
      opencodeConfigRoot({ env: p.env }),
    );
    if (!selected.ok) throw new Error(selected.reason);
    expect(selected.source).toBe("project");
    expect(selected.plugins.map((plugin) => plugin.name).toSorted()).toEqual([...enabledNames(p)]);
    expect(enabledNames(p)).toContain("ast-grep");
    expect(enabledNames(p)).toContain("delivery-flow");
    expect(enabledNames(p)).not.toContain("jev");
  });
});
