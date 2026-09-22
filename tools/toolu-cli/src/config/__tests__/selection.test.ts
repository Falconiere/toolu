import { afterAll, describe, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { UsageError } from "../../exit";
import { readSelection } from "../read";
import { SELECTION_VERSION } from "../schema";
import { selectionPath, writeSelection } from "../write";

const root = await mkdtemp(join(tmpdir(), "toolu-selection-"));
afterAll(async () => {
  await rm(root, { recursive: true, force: true });
});

async function fixture(name: string, body: string): Promise<string> {
  const path = join(root, name);
  await writeFile(path, body, "utf8");
  return path;
}

describe("readSelection", () => {
  test("reads a valid selection", async () => {
    const path = await fixture(
      "valid.json",
      JSON.stringify({ version: 1, host: "claude", enabled: ["toolu", "rust-quality"] }),
    );
    const selection = await readSelection(path, "6.5.0");
    expect(selection.enabled).toEqual(["toolu", "rust-quality"]);
    expect(selection.host).toBe("claude");
  });

  test("an empty enabled list is valid and means nothing selected", async () => {
    const path = await fixture("empty.json", JSON.stringify({ version: 1, enabled: [] }));
    expect((await readSelection(path, "6.5.0")).enabled).toEqual([]);
  });

  test("rejects an unsupported format version, naming both versions", async () => {
    const path = await fixture("v99.json", JSON.stringify({ version: 99, enabled: [] }));
    expect(readSelection(path, "6.5.0")).rejects.toThrow(UsageError);
    expect(readSelection(path, "6.5.0")).rejects.toThrow(
      new RegExp(`declares version 99.*understands version ${SELECTION_VERSION}`),
    );
  });

  test("rejects a file with no version field", async () => {
    const path = await fixture("noversion.json", JSON.stringify({ enabled: [] }));
    expect(readSelection(path, "6.5.0")).rejects.toThrow(/no numeric "version" field/);
  });

  test("rejects a file written by a newer CLI major", async () => {
    const path = await fixture(
      "newer.json",
      JSON.stringify({ version: 1, generatorVersion: "7.0.0", enabled: [] }),
    );
    expect(readSelection(path, "6.5.0")).rejects.toThrow(/Upgrade the CLI/);
  });

  test("accepts a file written by an older or equal CLI major", async () => {
    const path = await fixture(
      "older.json",
      JSON.stringify({ version: 1, generatorVersion: "5.9.9", enabled: ["toolu"] }),
    );
    expect((await readSelection(path, "6.5.0")).enabled).toEqual(["toolu"]);
  });

  test("rejects a malformed enabled list", async () => {
    const path = await fixture("bad.json", JSON.stringify({ version: 1, enabled: [1, 2] }));
    expect(readSelection(path, "6.5.0")).rejects.toThrow(/malformed/);
  });

  test("rejects non-JSON and a missing file distinctly", async () => {
    const path = await fixture("broken.json", "{not json");
    expect(readSelection(path, "6.5.0")).rejects.toThrow(/not valid JSON/);
    expect(readSelection(join(root, "absent.json"), "6.5.0")).rejects.toThrow(/cannot read/);
  });
});

describe("writeSelection", () => {
  test("writes to .toolu/plugins.json and round-trips through the reader", async () => {
    const path = selectionPath(root);
    await writeSelection(path, { version: 1, enabled: ["toolu"], host: "codex" });
    expect(path).toBe(join(root, ".toolu", "plugins.json"));
    expect((await readSelection(path, "6.5.0")).enabled).toEqual(["toolu"]);
    expect((await readFile(path, "utf8")).endsWith("\n")).toBe(true);
  });

  test("leaves no temporary file behind and overwrites cleanly", async () => {
    const path = join(root, "nested", "plugins.json");
    await writeSelection(path, { version: 1, enabled: ["a"] });
    await writeSelection(path, { version: 1, enabled: ["b"] });
    expect((await readSelection(path, "6.5.0")).enabled).toEqual(["b"]);
    const { readdir } = await import("node:fs/promises");
    expect((await readdir(join(root, "nested"))).sort()).toEqual(["plugins.json"]);
  });
});
