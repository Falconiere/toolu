import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "../sandbox.ts";

test.concurrent("a git sandbox has a committed HEAD on the requested branch", () => {
  using sb = createSandbox({
    git: true,
    branch: "development",
    files: { "src/a.ts": "export {};\n" },
  });
  expect(sb.git("rev-parse", "--abbrev-ref", "HEAD").trim()).toBe("development");
  expect(sb.git("ls-files").trim()).toBe("src/a.ts");
  expect(sb.git("status", "--porcelain")).toBe("");
});

test.concurrent("an empty git sandbox still gets an initial commit", () => {
  using sb = createSandbox({ git: true });
  expect(sb.git("rev-parse", "--abbrev-ref", "HEAD").trim()).toBe("main");
  expect(sb.git("ls-files").trim()).toBe(".gitkeep");
});

test.concurrent("concurrent sandboxes own disjoint roots and disposal removes each", () => {
  const roots: string[] = [];
  {
    using a = createSandbox();
    using b = createSandbox();
    roots.push(a.root, b.root);
    expect(a.root).not.toBe(b.root);
    expect(a.project.startsWith(a.root)).toBe(true);
    expect(existsSync(a.project) && existsSync(b.codexHome)).toBe(true);
  }
  expect(roots.some((root) => existsSync(root))).toBe(false);
});

test.concurrent("disposal runs when the test body throws", () => {
  let root = "";
  expect(() => {
    using sb = createSandbox();
    root = sb.root;
    throw new Error("boom");
  }).toThrow("boom");
  expect(existsSync(root)).toBe(false);
});

test.concurrent("disposing twice is a no-op", () => {
  const sb = createSandbox();
  sb[Symbol.dispose]();
  expect(() => sb[Symbol.dispose]()).not.toThrow();
});

test.concurrent("write encodes objects as JSON and read returns the body", () => {
  using sb = createSandbox();
  const abs = sb.write("nested/data.json", { a: 1 });
  expect(abs).toBe(join(sb.project, "nested/data.json"));
  expect(JSON.parse(sb.read("nested/data.json"))).toEqual({ a: 1 });
});

test.concurrent("paths that escape the project are refused", () => {
  using sb = createSandbox();
  expect(() => sb.path("../home/x")).toThrow("escapes");
  expect(() => sb.write("/etc/hosts", "x")).toThrow("escapes");
  expect(sb.path(".")).toBe(sb.project);
});

test.concurrent("writeConfig lands in each host's project and user config roots", () => {
  using sb = createSandbox();
  const cases = [
    [
      sb.writeConfig("claude", "project", { version: 1 }),
      join(sb.project, ".claude/toolu.config.json"),
    ],
    [
      sb.writeConfig("codex", "project", { version: 1 }),
      join(sb.project, ".codex/toolu.config.json"),
    ],
    [sb.writeConfig("claude", "user", { version: 1 }), join(sb.home, ".claude/toolu.config.json")],
    [sb.writeConfig("codex", "user", { version: 1 }), join(sb.codexHome, "toolu.config.json")],
    [sb.writeConfig("cursor", "user", { version: 1 }), join(sb.home, ".cursor/toolu.config.json")],
    [
      sb.writeConfig("opencode", "user", { version: 1 }),
      join(sb.home, ".config/opencode/toolu.config.json"),
    ],
  ];
  for (const [written, expected] of cases) {
    expect(written).toBe(expected);
    expect(JSON.parse(readFileSync(expected ?? "", "utf8"))).toEqual({ version: 1 });
  }
});
