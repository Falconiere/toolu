/**
 * Golden cases (#265) for file and function size: size.bats, plus the
 * threshold sources the spec names (toolu.config override, eslint and oxlint
 * `max-lines`, an unparsed eslint config, biome, the approximation note).
 */
import { TS_PROJECT, constLines, wrote, type TsCase } from "./cases-types.ts";

const FN_3 = { lang: { ts: { maxFnLines: 3 } } };

const fnCase = (
  name: string,
  file: string,
  body: string,
  flagged: boolean,
  max = FN_3,
): TsCase => ({
  name,
  project: TS_PROJECT,
  config: max,
  steps: wrote(file, body),
  expect: "advisory",
  ...(flagged ? { contains: ["Function too long"] } : { absent: ["Function too long"] }),
});

export const SIZE_CASES: readonly TsCase[] = [
  {
    name: "size: project config lowers maxFileLines",
    project: TS_PROJECT,
    config: { lang: { ts: { maxFileLines: 10 } } },
    steps: wrote("src/big.ts", constLines(15)),
    expect: "advisory",
    contains: ["exceeds 10-line limit"],
  },
  {
    name: "size: comments and blank lines do not count",
    project: TS_PROJECT,
    config: { lang: { ts: { maxFileLines: 10 } } },
    steps: wrote(
      "src/padded.ts",
      Array.from(
        { length: 8 },
        (_, i) =>
          `// explanatory comment ${String(i)}\n\nexport const v${String(i)} = ${String(i)};\n`,
      ).join(""),
    ),
    expect: "silent",
    absent: ["exceeds 10-line limit"],
  },
  {
    name: "size: eslint JSON max-lines is credited",
    project: { ...TS_PROJECT, ".eslintrc.json": '{"rules":{"max-lines":10}}\n' },
    steps: wrote("src/big.ts", constLines(15)),
    expect: "advisory",
    contains: ["eslint enforces this max-lines limit"],
  },
  {
    name: "size: oxlint max-lines object form is credited",
    project: { ...TS_PROJECT, ".oxlintrc.json": '{"rules":{"max-lines":["error",{"max":12}]}}\n' },
    steps: wrote("src/big.ts", constLines(15)),
    expect: "advisory",
    contains: ["exceeds 12-line limit", "oxc enforces this max-lines limit"],
  },
  {
    name: "size: unparsed .eslintrc.cjs falls to the default with a hint",
    project: { ...TS_PROJECT, ".eslintrc.cjs": "module.exports = { rules: {} };\n" },
    steps: wrote("src/huge.ts", constLines(301)),
    expect: "advisory",
    contains: ["didn't come from its config"],
  },
  {
    name: "size: eslint.config.js is not machine-readable either",
    project: { ...TS_PROJECT, "eslint.config.js": "export default [];\n" },
    steps: wrote("src/huge.ts", constLines(301)),
    expect: "advisory",
    contains: ["exceeds 300-line limit", "didn't come from its config"],
  },
  {
    name: "size: biome has no max-lines equivalent",
    project: { ...TS_PROJECT, "biome.json": "{}\n" },
    steps: wrote("src/huge.ts", constLines(301)),
    expect: "advisory",
    contains: ["biome has no max-lines equivalent"],
  },
  {
    name: "size: a toolu.config override beats the linter's limit",
    project: { ...TS_PROJECT, ".eslintrc.json": '{"rules":{"max-lines":50}}\n' },
    config: { lang: { ts: { maxFileLines: 10 } } },
    steps: wrote("src/big.ts", constLines(15)),
    expect: "advisory",
    contains: ["exceeds 10-line limit"],
    absent: ["enforces this max-lines limit"],
  },
  {
    name: "size: an unterminated block comment marks the count approximate",
    project: TS_PROJECT,
    config: { lang: { ts: { maxFileLines: 5 } } },
    steps: wrote("src/open.ts", `${constLines(8)}/* never closed\n`),
    expect: "advisory",
    contains: ["size approximated"],
  },
  fnCase(
    "size: long exported arrow const",
    "src/a.ts",
    "export const fn = () => {\n  const a = 1;\n  const b = 2;\n  const c = 3;\n  return a + b + c;\n};\n",
    true,
  ),
  fnCase(
    "size: one-line arrows before a function are not misattributed",
    "src/a.ts",
    "export const noop = () => undefined;\nexport const square = (x: number) => x * x;\nexport const upper = (s: string) => s.trim();\nexport function foo(): number {\n  const a = 1;\n  const b = 2;\n  return a + b;\n}\n",
    false,
    { lang: { ts: { maxFnLines: 5 } } },
  ),
  fnCase(
    "size: long function after one-line arrows",
    "src/a.ts",
    "export const noop = () => undefined;\nexport function foo(): number {\n  const a = 1;\n  const b = 2;\n  const c = 3;\n  const d = 4;\n  return a + b + c + d;\n}\n",
    true,
  ),
  fnCase(
    "size: long class method",
    "src/svc.ts",
    "export class Service {\n  process(x: number): number {\n    const a = x + 1;\n    const b = a + 1;\n    const c = b + 1;\n    const d = c + 1;\n    return d;\n  }\n}\n",
    true,
  ),
  fnCase(
    "size: short method before other members",
    "src/svc.ts",
    "export class Service {\n  ping(): number {\n    return 1;\n  }\n  a = 1;\n  b = 2;\n  c = 3;\n  d = 4;\n  e = 5;\n  f = 6;\n}\n",
    false,
  ),
  fnCase(
    "size: arrow with a multi-line param list",
    "src/a.ts",
    "export const compute = (\n  a: number,\n  b: number,\n  c: number,\n) => {\n  const sum = a + b + c;\n  return sum * 2;\n};\n",
    true,
  ),
  fnCase(
    "size: column-0 } in a template literal",
    "src/tmpl.ts",
    "export function render(): string {\n  const css = `\n.foo {\n  color: red;\n}\n`;\n  const a = 1;\n  const b = 2;\n  const c = 3;\n  return css + a + b + c;\n}\n",
    true,
    { lang: { ts: { maxFnLines: 5 } } },
  ),
  fnCase(
    "size: long const function expression",
    "src/a.ts",
    "export const fn = function() {\n  const a = 1;\n  const b = 2;\n  const c = 3;\n  return a + b + c;\n};\n",
    true,
  ),
  fnCase(
    "size: method with a defaulted generic",
    "src/svc.ts",
    "export class Service {\n  process<T = number>(x: T): T {\n    const a = x;\n    const b = a;\n    const c = b;\n    const d = c;\n    return d;\n  }\n}\n",
    true,
  ),
  fnCase(
    "size: strings and control flow do not start or end a function",
    "src/flow.ts",
    "export function run(x: number): string {\n  if (x > 1) {\n    return \"}\";\n  }\n  for (const y of [1]) {\n    void y;\n  }\n  return '{' + `}`;\n}\n",
    true,
  ),
];
