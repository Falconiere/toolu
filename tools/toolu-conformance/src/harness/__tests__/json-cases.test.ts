import { expect, test } from "bun:test";
import { readlinkSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "../sandbox.ts";
import {
  applyCaseSetup,
  assertCaptureNames,
  materializeCaseValue,
  readCaseFile,
  resolveFixturePath,
} from "../json-cases.ts";

test("a JSON case prepares a real git sandbox and keeps its name", () => {
  using sb = createSandbox({ git: true });
  const file = join(sb.root, "protected-file-cases.json");
  writeFileSync(
    file,
    JSON.stringify({
      version: 1,
      cases: [
        {
          name: "protected-files: asks on .env",
          setup: [
            { op: "write", path: "$PROJECT/.env", body: "SECRET=fixture\n" },
            { op: "git", args: ["add", ".env"] },
            { op: "git", args: ["commit", "-m", "add protected input"] },
          ],
          expect: "ask",
        },
      ],
    }),
  );
  const [fixture] = readCaseFile(file);
  expect(fixture?.name).toBe("protected-files: asks on .env");
  applyCaseSetup(sb, fixture?.setup ?? []);
  expect(sb.read(".env")).toBe("SECRET=fixture\n");
  expect(sb.git("log", "-1", "--format=%s").trim()).toBe("add protected input");
  applyCaseSetup(sb, [
    { op: "git", args: ["worktree", "add", "-q", "-b", "feat/fixture", { $path: "$ROOT/wt" }] },
  ]);
  expect(sb.git("worktree", "list", "--porcelain")).toContain(join(sb.root, "wt"));
  applyCaseSetup(sb, [
    { op: "write", path: "$ROOT/target", body: { $template: "project=$PROJECT" } },
    { op: "symlink", path: "$PROJECT/link", target: { $path: "$ROOT/target" } },
    { op: "write", path: "$PROJECT/bin/tool", body: "#!/bin/sh\n" },
    { op: "chmod", path: "$PROJECT/bin/tool", mode: "755" },
  ]);
  expect(sb.read("link")).toBe(`project=${sb.project}`);
  expect(statSync(sb.path("bin/tool")).mode & 0o777).toBe(0o755);
  expect(
    materializeCaseValue(sb, {
      file_path: { $path: "$PROJECT/.env" },
      command: "echo $HOME",
      repoCommand: { $template: "git -C $PROJECT status" },
    }),
  ).toEqual({
    file_path: sb.path(".env"),
    command: "echo $HOME",
    repoCommand: `git -C ${sb.project} status`,
  });
});

test("invalid records and operations fail before a hook runs", () => {
  using sb = createSandbox();
  const file = join(sb.root, "invalid.json");
  writeFileSync(file, '{"version":1,"cases":[{"name":"same"},{"name":"same"}]}');
  expect(() => readCaseFile(file)).toThrow("duplicate case name");
  writeFileSync(file, '{"version":2,"cases":[]}');
  expect(() => readCaseFile(file)).toThrow();
  expect(() => applyCaseSetup(sb, [{ op: "shell", command: "touch bad" }])).toThrow();
  expect(() => resolveFixturePath(sb, "$PROJECT/../../outside")).toThrow("escapes");
  expect(() => resolveFixturePath(sb, "$UNKNOWN/file")).toThrow("unknown path token");
  expect(() => resolveFixturePath(sb, "$REPO/package.json")).toThrow("unknown path token");
  applyCaseSetup(sb, [
    {
      op: "symlink",
      path: "$PROJECT/repo-jscpd",
      target: { $path: "$REPO/node_modules/.bin/jscpd" },
    },
  ]);
  expect(readlinkSync(sb.path("repo-jscpd"))).toBe(
    resolve(import.meta.dir, "../../../../../node_modules/.bin/jscpd"),
  );
  expect(() =>
    applyCaseSetup(sb, [{ op: "write", path: "$PROJECT/repo-jscpd", body: "bad" }]),
  ).toThrow("mutation follows symlink");
  expect(() =>
    applyCaseSetup(sb, [
      { op: "symlink", path: "$PROJECT/escape", target: { $path: "$REPO/package.json" } },
    ]),
  ).toThrow("unknown path token");
  expect(() =>
    applyCaseSetup(sb, [{ op: "symlink", path: "$PROJECT/escape", target: "/etc/passwd" }]),
  ).toThrow();
  expect(() => materializeCaseValue(sb, { $template: "echo $UNKNOWN" })).toThrow(
    "unknown path token",
  );
  expect(() => assertCaptureNames([{ name: "a" }, { name: "b" }], { a: {}, b: {} })).not.toThrow();
  expect(() => assertCaptureNames([{ name: "a" }, { name: "b" }], { a: {} })).toThrow(
    "capture names differ",
  );
});
