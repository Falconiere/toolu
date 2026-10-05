import { expect, test } from "bun:test";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
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
  expect(() => materializeCaseValue(sb, { $template: "echo $UNKNOWN" })).toThrow(
    "unknown path token",
  );
  expect(() => assertCaptureNames([{ name: "a" }, { name: "b" }], { a: {}, b: {} })).not.toThrow();
  expect(() => assertCaptureNames([{ name: "a" }, { name: "b" }], { a: {} })).toThrow(
    "capture names differ",
  );
});
