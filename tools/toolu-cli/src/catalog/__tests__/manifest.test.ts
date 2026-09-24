import { describe, expect, test } from "bun:test";
import { access } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { manifestCandidates, readMarketplace } from "../manifest";

const SRC = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const REPO_CATALOG = resolve(SRC, "../../../.claude-plugin/marketplace.json");

describe("manifestCandidates", () => {
  test("from source, only the repository's live catalog", async () => {
    expect(manifestCandidates(SRC)).toEqual([REPO_CATALOG]);
    await access(REPO_CATALOG);
    const catalog = await readMarketplace(REPO_CATALOG);
    expect(catalog.name).toBe("toolu");
  });

  // Builds before the CLI published from npm/ wrote tools/toolu-cli/assets/.
  // A source run must never pick that frozen copy over the live catalog.
  test("from source, never an assets/ copy a past build left beside src/", () => {
    expect(manifestCandidates(SRC)).not.toContain(resolve(SRC, "../assets/marketplace.json"));
  });

  test("from the published bundle, only the copy shipped beside dist/", () => {
    const dist = "/tmp/npx/node_modules/@toolu/plugins/dist";
    expect(manifestCandidates(dist)).toEqual([
      "/tmp/npx/node_modules/@toolu/plugins/assets/marketplace.json",
    ]);
  });
});
