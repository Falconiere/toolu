/**
 * Golden cases (#265) for error handling: error-handling.bats and
 * throw-literal.bats, including the ast-grep failure stages. The failing
 * `ast-grep` is a stub script on PATH, the only way to make the real tool
 * crash on demand.
 */
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { pathWithout } from "./cases-path.ts";
import { TS_PROJECT, wrote, type Step, type TsCase } from "./cases-types.ts";

/** PATH with a stub `ast-grep` first, as the bats suites did. */
export function stubAstGrep(body: string): (sb: Sandbox) => { PATH: string } {
  return (sb) => {
    const bin = join(sb.root, "stub-bin");
    mkdirSync(bin, { recursive: true });
    writeFileSync(join(bin, "ast-grep"), `#!/bin/sh\n${body}\n`);
    chmodSync(join(bin, "ast-grep"), 0o755);
    return { PATH: `${bin}:${process.env.PATH ?? ""}` };
  };
}

const edit = (file: string, body: string): Step[] => wrote(file, body, "Edit");

const err = (
  name: string,
  steps: Step[],
  check: Pick<TsCase, "contains" | "absent" | "expect">,
): TsCase => ({
  name,
  project: TS_PROJECT,
  steps,
  ...check,
});

const flags = (...text: string[]) => ({ expect: "advisory" as const, contains: text });
const passes = (text: string) => ({ expect: "advisory" as const, absent: [text] });
const quiet = (text: string) => ({ expect: "silent" as const, absent: [text] });

const EMPTY_CATCH = "export function bad() {\n  try {\n    foo();\n  } catch (e) {}\n}\n";
const CLEAN = "/** clean */\nexport function good() {\n  return 1;\n}\n";

export const ERROR_CASES: readonly TsCase[] = [
  err("errors: empty catch block", edit("src/bad.ts", EMPTY_CATCH), flags("Empty catch block")),
  err(
    "errors: empty catch without a binding",
    edit("src/bad.ts", "export function bad() {\n  try { foo(); } catch { }\n}\n"),
    flags("Empty catch block"),
  ),
  err(
    "errors: silent .catch(() => {})",
    edit(
      "src/bad.ts",
      "export function bad() {\n  foo().catch(() => {});\n  bar().catch(() => null);\n  baz().catch(() => undefined);\n}\n",
    ),
    flags("Silent promise rejection"),
  ),
  err(
    "errors: clean error handling",
    edit(
      "src/good.ts",
      'export function good() {\n  try {\n    foo();\n  } catch (e) {\n    console.error(e);\n    throw e;\n  }\n  foo().catch((err) => console.error(err));\n  throw new Error("descriptive");\n  throw new TypeError("typed");\n}\n',
    ),
    passes("QUALITY VIOLATION"),
  ),
  err(
    "errors: try/catch+toast in a component",
    edit(
      "src/components/save-button.tsx",
      'export function SaveButton() {\n  try {\n    save();\n  } catch (error) {\n    toast("save failed");\n  }\n}\n',
    ),
    flags("Manual try/catch+toast"),
  ),
  err(
    "errors: commented-out await",
    edit("src/a.ts", "// await legacyCall() — kept for reference\nexport const flag = true;\n"),
    quiet("uses await with no try/catch"),
  ),
  err(
    "errors: try only in a comment",
    edit(
      "src/a.ts",
      "// callers should try to handle this\nexport async function fetchIt(): Promise<number> {\n  const r = await doFetch();\n  return r;\n}\n",
    ),
    flags("uses await with no try/catch"),
  ),
  err(
    "errors: catch returns null",
    wrote(
      "src/bad.ts",
      "export function f() {\n  try {\n    risky();\n  } catch (e) {\n    return null;\n  }\n}\n",
    ),
    flags("swallows the error"),
  ),
  err(
    "errors: catch returns undefined, both bindings",
    wrote(
      "src/bad.ts",
      "export function f() {\n  try { a(); } catch (e) { return undefined }\n  try { b(); } catch { return undefined }\n  try { c(); } catch { return null }\n}\n",
    ),
    flags("swallows the error"),
  ),
  err(
    "errors: bare return in catch",
    wrote(
      "src/bad.ts",
      "export function f() {\n  try {\n    risky();\n  } catch {\n    return;\n  }\n}\n",
    ),
    flags("swallows the error"),
  ),
  err(
    "errors: catch returning a non-nullish value",
    wrote(
      "src/ok.ts",
      "/** Read a list, degrading to empty. */\nexport function readList(): string[] {\n  try {\n    return risky();\n  } catch {\n    return [];\n  }\n}\n/** Count things, degrading to zero. */\nexport function count(): number {\n  try {\n    return risky();\n  } catch (e) {\n    return 0;\n  }\n}\n",
    ),
    quiet("swallows the error"),
  ),
  err(
    "errors: excerpts on real newlines",
    edit("src/bad.ts", EMPTY_CATCH),
    flags("Empty catch block in", "/src/bad.ts:4:"),
  ),
  err(
    "errors: await with no handler is an advisory",
    wrote(
      "src/a.ts",
      '/** Fetch a thing. */\nexport async function f() {\n  const r = await fetchThing("x");\n  return r;\n}\n',
    ),
    {
      expect: "advisory",
      contains: ["uses await with no try/catch"],
      absent: ["QUALITY VIOLATION"],
    },
  ),
  err(
    "errors: await inside try/catch",
    wrote(
      "src/a.ts",
      '/** Fetch a thing safely. */\nexport async function f() {\n  try {\n    return await fetchThing("x");\n  } catch (e) {\n    throw new Error("fetch failed", { cause: e });\n  }\n}\n',
    ),
    { expect: "silent", absent: ["uses await with no try/catch"] },
  ),
  {
    ...err(
      "errors: ast-grep crash is a violation",
      wrote("src/good.ts", CLEAN),
      flags("ast-grep failed", "boom", "exit 2"),
    ),
    env: stubAstGrep("echo boom >&2\nexit 2"),
  },
  {
    ...err(
      "errors: ast-grep exit 0 with non-JSON output",
      wrote("src/good.ts", CLEAN),
      flags("did not parse as the documented JSON array"),
    ),
    env: stubAstGrep("echo 'not json'"),
  },
  {
    ...err("errors: ast-grep exit 0 with empty output passes", wrote("src/good.ts", CLEAN), {
      expect: "silent",
      absent: ["ast-grep failed"],
    }),
    env: stubAstGrep("exit 0"),
  },
  {
    ...err("errors: ast-grep absent skips the structural rules", edit("src/bad.ts", EMPTY_CATCH), {
      expect: "advisory",
      absent: ["Empty catch block"],
    }),
    env: pathWithout("ast-grep", "sg"),
  },
  err(
    "throw: new Error() with no message",
    edit("src/bad.ts", "export function bad() {\n  throw new Error();\n}\n"),
    flags("no message"),
  ),
  err(
    "throw: string literal",
    edit("src/bad.ts", 'export function bad() {\n  throw "bad";\n}\n'),
    flags("string literal"),
  ),
  err(
    "throw: template literal",
    edit("src/bad.ts", "export function bad() {\n  throw `bad`;\n}\n"),
    flags("string literal"),
  ),
  err(
    "throw: numeric literal",
    edit("src/bad.ts", "export function bad() {\n  throw 42;\n}\n"),
    flags("non-Error literal"),
  ),
  err(
    "throw: null",
    edit("src/bad.ts", "export function bad() {\n  throw null;\n}\n"),
    flags("non-Error literal"),
  ),
  err(
    "throw: negative float, booleans and undefined, capped at three",
    edit(
      "src/bad.ts",
      "function a() { throw -1.5; }\nfunction b() { throw true }\nfunction c() {throw false;}\nfunction d() { throw undefined; }\n",
    ),
    flags("non-Error literal"),
  ),
  err(
    "throw: MultiEdit names the file",
    wrote("src/bad.ts", "export function bad() { throw 42; }\n", "MultiEdit"),
    flags("non-Error literal"),
  ),
  err(
    "throw: inside a // comment",
    edit(
      "src/good.ts",
      "export function good() {\n  const x = 1; // we used to throw 5 here\n  return x;\n}\n",
    ),
    passes("non-Error literal"),
  ),
  err(
    "throw: inside a /* */ comment",
    edit(
      "src/good.ts",
      "export function good() {\n  /* avoid: throw 42 — use Error */\n  return 1;\n}\n",
    ),
    passes("non-Error literal"),
  ),
  err(
    "throw: part of a longer identifier",
    edit("src/good.ts", "export function good() {\n  rethrow 42;\n  $throw 1;\n}\n"),
    passes("non-Error literal"),
  ),
];
