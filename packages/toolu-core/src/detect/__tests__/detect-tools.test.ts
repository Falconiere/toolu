/**
 * Tool availability against bash `command -v` (#254 AC-5): `detect_ast_grep`
 * under a PATH of real executables linked in as `sg` only, `ast-grep` only,
 * both, and neither, plus the probe cache following a PATH change.
 */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { detectAstGrep, toolAvailable } from "../detect-tools.ts";
import { bashDetect, detectEnv } from "./detect-bash.ts";

const TRUE = Bun.which("true") ?? "/usr/bin/true";

/** A bin dir holding real executables under `names`, plus bash and the tools detect.sh sources with. */
function binDir(root: string, label: string, names: readonly string[]): string {
  const dir = join(root, label);
  mkdirSync(dir, { recursive: true });
  for (const name of names) symlinkSync(TRUE, join(dir, name));
  return dir;
}

const SYSTEM = "/usr/bin:/bin";

test.concurrent.each([
  ["neither", []],
  ["sg only", ["sg"]],
  ["ast-grep only", ["ast-grep"]],
  ["both", ["sg", "ast-grep"]],
] as const)("%s: detectAstGrep equals bash detect_ast_grep", async (label, names) => {
  using sb = createSandbox();
  const bin = binDir(sb.root, label.replaceAll(" ", "-"), names);
  const env = detectEnv(sb.home, { PATH: `${bin}:${SYSTEM}` });
  const bash =
    (await bashDetect("detect_ast_grep; printf .", [], sb.project, env)) === "ast-grep\n.";
  expect(detectAstGrep(env)).toBe(bash);
  // The system dirs may hold an unrelated `sg` (shadow-utils on Linux), so the
  // absolute answer is checked against the probe dir alone.
  expect(detectAstGrep({ PATH: bin })).toBe(names.length > 0);
});

test("toolAvailable matches command -v for executables, non-executables, directories and missing names", async () => {
  using sb = createSandbox();
  const bin = binDir(sb.root, "bin", ["exe"]);
  writeFileSync(join(bin, "plain"), "#!/bin/sh\n");
  chmodSync(join(bin, "plain"), 0o644);
  mkdirSync(join(bin, "adir"));
  symlinkSync(join(sb.root, "missing"), join(bin, "dangling"));
  const env = detectEnv(sb.home, { PATH: `${bin}:${SYSTEM}` });
  const names = [
    "exe",
    "plain",
    "adir",
    "dangling",
    "nope",
    "git",
    "",
    join(bin, "exe"),
    join(bin, "plain"),
    join(bin, "adir"),
  ];
  const out = await bashDetect(
    `for n in "$@"; do if [ -n "$n" ] && command -v "$n" >/dev/null 2>&1; then printf 1; else printf 0; fi; done`,
    names,
    sb.project,
    env,
  );
  expect(names.map((n) => (toolAvailable(n, env) ? "1" : "0")).join("")).toBe(out);
  expect(toolAvailable("exe", env)).toBe(true);
});

test("the probe cache is keyed by PATH, so a new PATH re-probes", () => {
  using sb = createSandbox();
  const without = binDir(sb.root, "without", []);
  const withSg = binDir(sb.root, "with", ["sg"]);
  expect(detectAstGrep({ PATH: without })).toBe(false);
  expect(detectAstGrep({ PATH: withSg })).toBe(true);
  expect(detectAstGrep({ PATH: without })).toBe(false);
  expect(toolAvailable("sg", { PATH: "" })).toBe(false);
});
