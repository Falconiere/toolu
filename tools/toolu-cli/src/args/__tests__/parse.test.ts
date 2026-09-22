import { describe, expect, test } from "bun:test";
import { EXIT, UsageError } from "../../exit";
import { parseArgs } from "../parse";

describe("parseArgs", () => {
  test("reads a full plugins install invocation", () => {
    const args = parseArgs([
      "plugins",
      "install",
      "toolu",
      "rust-quality",
      "--host",
      "claude",
      "--scope",
      "user",
      "--yes",
    ]);
    expect(args.noun).toBe("plugins");
    expect(args.verb).toBe("install");
    expect(args.names).toEqual(["toolu", "rust-quality"]);
    expect(args.host).toBe("claude");
    expect(args.scope).toBe("user");
    expect(args.yes).toBe(true);
    expect(args.dryRun).toBe(false);
  });

  test("reads the agents noun and its verbs", () => {
    expect(parseArgs(["agents", "preview"]).verb).toBe("preview");
    expect(parseArgs(["agents", "remove", "--yes"]).yes).toBe(true);
  });

  test("--version and --help parse without requiring a verb", () => {
    expect(parseArgs(["--version"]).version).toBe(true);
    expect(parseArgs(["--help"]).help).toBe(true);
    expect(parseArgs(["plugins", "--help"]).help).toBe(true);
  });

  test.each([
    ["unknown noun", ["skills", "install"]],
    ["unknown plugins verb", ["plugins", "sync"]],
    ["unknown agents verb", ["agents", "update"]],
    ["missing verb", ["plugins"]],
    ["unknown flag", ["plugins", "install", "--turbo"]],
    ["value flag without a value", ["plugins", "install", "--host"]],
    ["value flag followed by a flag", ["plugins", "install", "--host", "--yes"]],
    ["unknown host", ["plugins", "install", "--host", "cursor"]],
    ["unknown scope", ["plugins", "install", "--scope", "global"]],
  ])("rejects %s with exit %i", (_label, argv) => {
    expect(() => parseArgs(argv)).toThrow(UsageError);
    try {
      parseArgs(argv);
    } catch (error) {
      expect((error as UsageError).code).toBe(EXIT.usage);
    }
  });

  test("rejects --scope on a host that has no scope concept", () => {
    for (const host of ["codex", "opencode"]) {
      expect(() => parseArgs(["plugins", "install", "--scope", "user", "--host", host])).toThrow(
        /--scope is Claude Code only/,
      );
    }
  });

  test("accepts --scope when the host is claude or unspecified", () => {
    expect(parseArgs(["plugins", "install", "--scope", "user", "--host", "claude"]).scope).toBe(
      "user",
    );
    expect(parseArgs(["plugins", "install", "--scope", "project"]).scope).toBe("project");
  });

  test("an empty argv yields no noun rather than throwing", () => {
    const args = parseArgs([]);
    expect(args.noun).toBeUndefined();
    expect(args.names).toEqual([]);
  });
});
