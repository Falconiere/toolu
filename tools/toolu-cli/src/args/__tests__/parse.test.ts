import { describe, expect, test } from "bun:test";
import { EXIT, UsageError } from "../../exit";
import { assertScopeAllowed, parseArgs } from "../parse";

describe("parseArgs", () => {
  test("reads a full install invocation", () => {
    const args = parseArgs([
      "install",
      "toolu",
      "rust-quality",
      "--host",
      "claude",
      "--scope",
      "user",
      "--yes",
    ]);
    expect(args.verb).toBe("install");
    expect(args.names).toEqual(["toolu", "rust-quality"]);
    expect(args.host).toBe("claude");
    expect(args.scope).toBe("user");
    expect(args.yes).toBe(true);
    expect(args.dryRun).toBe(false);
  });

  test("reads every verb as the first argument, the way npx @toolu/plugins passes it", () => {
    for (const verb of ["install", "list", "remove", "update"] as const) {
      expect(parseArgs([verb]).verb).toBe(verb);
    }
  });

  test("--version and --help parse without requiring a verb", () => {
    expect(parseArgs(["--version"]).version).toBe(true);
    expect(parseArgs(["--help"]).help).toBe(true);
    expect(parseArgs(["install", "--help"]).help).toBe(true);
  });

  test.each([
    ["unknown verb", ["sync"]],
    ["the plugins noun of the old @toolu/cli", ["plugins", "install"]],
    ["the agents noun, which was never built", ["agents", "preview"]],
    ["unknown flag", ["install", "--turbo"]],
    ["value flag without a value", ["install", "--host"]],
    ["value flag followed by a flag", ["install", "--host", "--yes"]],
    ["unknown host", ["install", "--host", "cursor"]],
    ["unknown scope", ["install", "--scope", "global"]],
  ])("rejects %s with exit 2", (_label, argv) => {
    expect(() => parseArgs(argv)).toThrow(UsageError);
    try {
      parseArgs(argv);
    } catch (error) {
      expect((error as UsageError).code).toBe(EXIT.usage);
    }
  });

  test("names every valid verb when the first argument is not one", () => {
    expect(() => parseArgs(["plugins", "install"])).toThrow(
      "unknown command: plugins. Expected one of: install, list, remove, update",
    );
  });

  test("rejects --scope on a host that has no scope concept", () => {
    for (const host of ["codex", "opencode"]) {
      expect(() => parseArgs(["install", "--scope", "user", "--host", host])).toThrow(
        /--scope is Claude Code only/,
      );
    }
  });

  test("accepts --scope when the host is claude or unspecified", () => {
    expect(parseArgs(["install", "--scope", "user", "--host", "claude"]).scope).toBe("user");
    expect(parseArgs(["install", "--scope", "project"]).scope).toBe("project");
  });

  test("an empty argv yields no verb rather than throwing", () => {
    const args = parseArgs([]);
    expect(args.verb).toBeUndefined();
    expect(args.names).toEqual([]);
  });
});

describe("review-driven behavior", () => {
  test("an unknown verb is a usage error even alongside --help or --version", () => {
    expect(() => parseArgs(["skills", "--help"])).toThrow(/unknown command: skills/);
    expect(() => parseArgs(["skills", "--version"])).toThrow(/unknown command: skills/);
  });

  test("--help on a known verb, and bare --help, still answer", () => {
    expect(parseArgs(["install", "--help"]).verb).toBe("install");
    expect(parseArgs(["--help"]).verb).toBeUndefined();
  });

  test("--scope without --host parses, and is re-checked once the host resolves", () => {
    expect(parseArgs(["install", "--scope", "user"]).scope).toBe("user");
    expect(() => assertScopeAllowed("user", "claude")).not.toThrow();
    expect(() => assertScopeAllowed("user", "codex")).toThrow(/Claude Code only/);
    expect(() => assertScopeAllowed("user", "opencode")).toThrow(/Claude Code only/);
    expect(() => assertScopeAllowed(undefined, "codex")).not.toThrow();
  });
});
