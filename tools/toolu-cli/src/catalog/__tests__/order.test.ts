import { describe, expect, test } from "bun:test";
import { resolve } from "node:path";
import { UsageError } from "../../exit";
import { readMarketplace } from "../manifest";
import { catalogNames, dependentsOf, installOrder } from "../order";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const MANIFEST = resolve(REPO_ROOT, ".claude-plugin/marketplace.json");

const marketplace = await readMarketplace(MANIFEST);

describe("catalog against the real .claude-plugin/marketplace.json", () => {
  test("reads every catalog plugin", () => {
    expect(catalogNames(marketplace)).toContain("toolu");
    expect(catalogNames(marketplace).length).toBeGreaterThanOrEqual(13);
  });

  test("the four declared dependents really depend on toolu", () => {
    expect([...dependentsOf(marketplace, "toolu")].sort()).toEqual([
      "pr-babysit",
      "python-quality",
      "rust-quality",
      "ts-quality",
    ]);
  });

  test("a full install orders every dependent after toolu", () => {
    const order = installOrder(marketplace, []);
    expect(order.length).toBe(catalogNames(marketplace).length);
    const core = order.indexOf("toolu");
    for (const dependent of dependentsOf(marketplace, "toolu")) {
      expect(order.indexOf(dependent)).toBeGreaterThan(core);
    }
  });

  test("requesting only a dependent pulls its dependency in first", () => {
    expect(installOrder(marketplace, ["rust-quality"])).toEqual(["toolu", "rust-quality"]);
  });

  test("a standalone plugin brings nothing else along", () => {
    expect(installOrder(marketplace, ["jira"])).toEqual(["jira"]);
  });

  test("a repeated name appears once", () => {
    expect(installOrder(marketplace, ["toolu", "rust-quality", "toolu"])).toEqual([
      "toolu",
      "rust-quality",
    ]);
  });

  test("an unknown name is rejected before anything is ordered", () => {
    expect(() => installOrder(marketplace, ["not-a-plugin"])).toThrow(UsageError);
    expect(() => installOrder(marketplace, ["not-a-plugin"])).toThrow(/unknown plugin/);
  });
});

describe("manifest validation", () => {
  test("a missing file fails with a readable message", async () => {
    expect(readMarketplace(resolve(REPO_ROOT, "no-such-manifest.json"))).rejects.toThrow(
      /cannot read marketplace manifest/,
    );
  });

  test("a non-manifest JSON file is rejected as malformed", async () => {
    expect(readMarketplace(resolve(REPO_ROOT, "package.json"))).rejects.toThrow(/malformed/);
  });

  test("a non-JSON file is rejected as invalid JSON", async () => {
    expect(readMarketplace(resolve(REPO_ROOT, "README.md"))).rejects.toThrow(/not valid JSON/);
  });
});
