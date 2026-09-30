/**
 * Golden cases (#265) for the line rules: type-safety, suppression, ui-bans
 * and docs bats suites, plus one case per fragment no suite covered (React
 * hooks, factories, type guards, naming, console, confirm/alert, props).
 */
import { ALIAS_PROJECT, TS_PROJECT, wrote, type TsCase } from "./cases-types.ts";

const rule = (
  name: string,
  file: string,
  body: string,
  check: Pick<TsCase, "contains" | "absent" | "expect">,
  project: TsCase["project"] = TS_PROJECT,
): TsCase => ({ name, project, steps: wrote(file, body), ...check });

const flags = (text: string) => ({ expect: "advisory" as const, contains: [text] });
const passes = (text: string) => ({ expect: "advisory" as const, absent: [text] });
const quiet = (text: string) => ({ expect: "silent" as const, absent: [text] });

const AS = "Forbidden 'as' type assertion";
const SUPPRESS = "Forbidden suppression comment";
const DOC = "missing a JSDoc";

export const RULE_CASES: readonly TsCase[] = [
  rule(
    "as: export { foo as Bar } is a re-export",
    "src/reexport.ts",
    'import { foo } from "@/foo";\nexport { foo as Bar };\n',
    quiet(AS),
  ),
  rule("as: export const x = foo as Bar", "src/a.ts", "export const x = foo as Bar;\n", flags(AS)),
  rule(
    "as: as null / as void",
    "src/a.ts",
    "const a = something as null;\nconst b = other as void;\n",
    flags(AS),
  ),
  rule(
    "as: const, commented and import lines are exempt; the rest capped at five",
    "src/a.ts",
    'import { a as b } from "x";\n// y as Foo\nconst k = [1] as const;\nconst a1 = v as A;\nconst a2 = v as B;\nconst a3 = (v) as C;\nconst a4 = v as any;\nconst a5 = v as unknown;\nconst a6 = v as string;\n',
    flags(AS),
  ),
  {
    name: "type-dup: an exported type already in another package",
    project: TS_PROJECT,
    commit: { "packages/a/src/widget.ts": "export interface Widget { id: string }\n" },
    steps: wrote("packages/b/src/widget2.ts", "export interface Widget { id: string }\n"),
    expect: "advisory",
    contains: ["already defined in packages/a/src/widget.ts"],
  },
  rule(
    "suppression: eslint-disable",
    "src/bad.ts",
    "// eslint-disable-next-line\nexport const a = thing();\n",
    flags(SUPPRESS),
  ),
  rule(
    "suppression: @ts-expect-error outside tests",
    "src/bad.ts",
    "// @ts-expect-error legacy boundary\nexport const b = thing();\n",
    flags(SUPPRESS),
  ),
  {
    name: "suppression: @ts-expect-error allowed in a test file",
    project: TS_PROJECT,
    commit: { "src/code.ts": "export const code = 1;\n" },
    steps: wrote(
      "src/__tests__/code.test.ts",
      "// @ts-expect-error asserting a type error is the point of this test\nconst z: string = 123;\n",
    ),
    expect: "silent",
    absent: [SUPPRESS],
  },
  rule(
    "suppression: @ts-ignore in a /** */ block",
    "src/bad.ts",
    "/** @ts-ignore */\nexport const a = thing();\n",
    flags(SUPPRESS),
  ),
  rule(
    "suppression: biome-ignore and @ts-nocheck, capped at three",
    "src/bad.ts",
    "// @ts-nocheck\n/* biome-ignore lint: x */\n// @ts-ignore\n// eslint-disable\n",
    flags(SUPPRESS),
  ),
  rule(
    "radix: raw import",
    "src/a.ts",
    'import { Dialog } from "@radix-ui/react-dialog";\nexport const x = 1;\n',
    flags("Raw radix import"),
  ),
  rule(
    "radix: only in a comment",
    "src/a.ts",
    '// import { Dialog } from "@radix-ui/react-dialog";\nexport const x = 1;\n',
    quiet("Raw radix import"),
  ),
  rule(
    "radix: allowed under packages/ui",
    "packages/ui/src/dialog.ts",
    "import * as D from '@radix-ui/react-alert-dialog';\nexport const x = D;\n",
    quiet("Raw radix import"),
  ),
  rule(
    "docs: verbose JSDoc with /** in prose",
    "src/api.ts",
    `/**\n * Does the thing.\n${Array.from({ length: 11 }, (_, i) => ` * line ${String(i + 3)}${i === 2 ? " use a /** block for docs, they said" : ""}\n`).join("")} */\nexport function doThing() {\n  return 1;\n}\n`,
    flags("JSDoc block is"),
  ),
  rule(
    "docs: exported function without JSDoc",
    "src/api.ts",
    "export function doThing() {\n  return 1;\n}\n",
    { expect: "advisory", contains: [DOC], absent: ["QUALITY VIOLATION"] },
  ),
  rule(
    "docs: pragma between JSDoc and export",
    "src/api.ts",
    "/** Does the thing. */\n// eslint-disable-next-line @typescript-eslint/no-explicit-any\nexport function doThing() {\n  return 1;\n}\n",
    passes(DOC),
  ),
  rule(
    "docs: documented export",
    "src/api.ts",
    "/** Does the thing. */\nexport function doThing() {\n  return 1;\n}\n",
    { expect: "silent", absent: [DOC] },
  ),
  rule(
    "docs: camelCase exported arrow",
    "src/a.ts",
    "export const myApi = () => {\n  return 1;\n};\n",
    flags(DOC),
  ),
  rule(
    "docs: index.ts and .d.ts are exempt",
    "src/index.ts",
    "export function doThing() {\n  return 1;\n}\n",
    { expect: "silent", absent: [DOC] },
  ),
  rule(
    "docs: class, enum, default and interface exports",
    "src/kinds.ts",
    "export class A {}\nexport enum B { X }\nexport default 1;\nexport interface C { x: number }\nexport type D = number;\n",
    flags(DOC),
  ),
  rule(
    "hooks: a hook file doing too many things",
    "src/use-thing.ts",
    "/** Hook. */\nexport function useThing() {\n  const [a] = x;\n  const [b] = x;\n  useRef(1);\n  useEffect(() => a + b);\n}\n",
    flags("Hook does too many things"),
  ),
  rule(
    "factory: more than two create functions",
    "src/make.ts",
    "/** a */\nexport function createA() {}\n/** b */\nexport async function createB() {}\n/** c */\nexport function createC() {}\n",
    flags("Too many factory functions"),
  ),
  rule(
    "typeguard: manual guard in a zod project",
    "src/guard.ts",
    "/** guard */\nexport function isFoo(x: unknown): x is Foo {\n  return x !== null;\n}\n",
    flags("Manual type guard"),
    { ...TS_PROJECT, "package.json": '{"name":"x","dependencies":{"zod":"4"}}\n' },
  ),
  rule(
    "typeguard: no zod, no rule",
    "src/guard.ts",
    "/** guard */\nexport function isFoo(x: unknown): x is Foo {\n  return x !== null;\n}\n",
    { expect: "silent", absent: ["Manual type guard"] },
  ),
  rule(
    "naming: parts.tsx",
    "src/components/parts.tsx",
    "/** p */\nexport function Parts() {\n  return null;\n}\n",
    flags("Forbidden component filename"),
  ),
  rule(
    "naming: card-sections.tsx",
    "src/card-sections.tsx",
    "/** p */\nexport function CardSections() {\n  return null;\n}\n",
    flags("Forbidden component filename"),
  ),
  rule(
    "console: console.log, capped at three",
    "src/log.ts",
    'console.log("a");\n  console.log("b");\n// console.log("c");\nconsole.log("d");\nconsole.log("e");\n',
    flags("Forbidden console.log"),
  ),
  rule(
    "confirm: confirm()/alert() in a component",
    "src/components/del.tsx",
    '/** d */\nexport function Del() {\n  if (confirm("sure?")) alert("gone");\n  // confirm("no")\n  customConfirm("fine");\n  return <AlertDialog />;\n}\n',
    flags("Forbidden confirm()/alert()"),
  ),
  rule(
    "confirm: only under components/ or routes/",
    "src/lib/del.ts",
    '/** d */\nexport function del() {\n  return confirm("sure?");\n}\n',
    { expect: "silent", absent: ["confirm()/alert()"] },
  ),
  rule(
    "props: mutable props",
    "src/card.tsx",
    "/** c */\nexport function Card(props: CardProps) {\n  return null;\n}\nfunction Row(rowProps: RowProps) {}\nfunction Ok(props: Readonly<OkProps>) {}\n",
    flags("Mutable props"),
  ),
  rule(
    "imports: ../ import with a @/ alias",
    "src/deep/a.ts",
    'import { thing } from "../other";\nexport const x = thing;\n',
    flags("Forbidden ../ import"),
    ALIAS_PROJECT,
  ),
];
