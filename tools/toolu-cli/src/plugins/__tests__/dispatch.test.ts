import { afterAll, describe, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "../../args/parse";
import { EXIT } from "../../exit";
import { dispatchPlugins } from "../dispatch";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const manifestPath = resolve(REPO_ROOT, ".claude-plugin/marketplace.json");

const stubDir = await mkdtemp(join(tmpdir(), "toolu-dispatch-hosts-"));
await writeFile(join(stubDir, "claude"), "#!/bin/sh\n", { mode: 0o755 });
await writeFile(join(stubDir, "codex"), "#!/bin/sh\n", { mode: 0o755 });
await chmod(join(stubDir, "claude"), 0o755);
await chmod(join(stubDir, "codex"), 0o755);
const stubEnv = { ...process.env, PATH: stubDir };
afterAll(async () => {
  await rm(stubDir, { recursive: true, force: true });
});

describe("dispatchPlugins install", () => {
  test("dry-run multi-host install loops both hosts and sections the report", async () => {
    const chunks: string[] = [];
    const args = parseArgs(["install", "--dry-run"]);
    if (args.verb === undefined) throw new Error("verb missing");
    const code = await dispatchPlugins(
      { ...args, verb: args.verb },
      {
        manifestPath,
        interactive: true,
        env: stubEnv,
        write: (text) => {
          chunks.push(text);
        },
        selectHosts: async () => ["claude", "codex"],
        selectPlugins: async () => ["jira"],
      },
    );
    expect(code).toBe(EXIT.ok);
    const text = chunks.join("");
    expect(text).toContain("claude:");
    expect(text).toContain("codex:");
    expect(text).toContain("jira");
  });

  test("non-interactive multi-host install still exits 3", async () => {
    const args = parseArgs(["install", "--dry-run", "--no-input"]);
    if (args.verb === undefined) throw new Error("verb missing");
    await expect(
      dispatchPlugins(
        { ...args, verb: args.verb },
        {
          manifestPath,
          interactive: false,
          env: stubEnv,
          write: () => undefined,
        },
      ),
    ).rejects.toMatchObject({ code: EXIT.missingInput, message: expect.stringMatching(/several hosts/) });
  });

  test("named plugins skip the plugin picker", async () => {
    let pluginsAsked = false;
    const chunks: string[] = [];
    const args = parseArgs(["install", "jira", "--dry-run", "--host", "claude"]);
    if (args.verb === undefined) throw new Error("verb missing");
    const code = await dispatchPlugins(
      { ...args, verb: args.verb },
      {
        manifestPath,
        interactive: true,
        env: stubEnv,
        write: (text) => {
          chunks.push(text);
        },
        selectPlugins: async () => {
          pluginsAsked = true;
          return ["toolu"];
        },
      },
    );
    expect(code).toBe(EXIT.ok);
    expect(pluginsAsked).toBe(false);
    expect(chunks.join("")).toContain("jira");
  });

  test("cancelling the plugin picker returns exit 130", async () => {
    const args = parseArgs(["install", "--dry-run", "--host", "claude"]);
    if (args.verb === undefined) throw new Error("verb missing");
    const { CliError } = await import("../../exit");
    const code = await dispatchPlugins(
      { ...args, verb: args.verb },
      {
        manifestPath,
        interactive: true,
        env: stubEnv,
        write: () => undefined,
        selectPlugins: async () => {
          throw new CliError(EXIT.cancelled, "cancelled");
        },
      },
    );
    expect(code).toBe(EXIT.cancelled);
  });
});
