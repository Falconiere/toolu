// Ported from the upstream kit's run-fixtures.sh AC-23: every suppression form
// the lexer must see through, and every documentation form it must leave legal.
// Each source is written into a real fixture repo and scanned by the runner.
import { expect, test } from "bun:test";
import { buildFixture } from "./fixture-tree.ts";
import { count, gr } from "./gr-harness.ts";

/** [file under src/utilities/, source, forbidden?] */
type Case = readonly [string, string, boolean];

const DIRECTIVE_CASES: readonly Case[] = [
  [
    "suppression-notes.ts",
    "// Documentation: oxlint-disable-next-line eslint/no-unused-vars is forbidden.\n",
    false,
  ],
  ["blanket-disable.ts", "/* oxlint-disable */ const hidden = 1;\n", true],
  [
    "scoped-disable.ts",
    '// oxlint-disable-next-line no-console -- intentional test probe\nconsole.log("probe");\n',
    false,
  ],
  ["scoped-block-disable.ts", '/* oxlint-disable no-console */ console.log("probe");\n', false],
  [
    "aliased-unused-disable.ts",
    "// eslint-disable-next-line no-console, @typescript-eslint/no-unused-vars\nconst hidden = 1;\n",
    true,
  ],
  ["trailing-unused-disable.ts", "const hidden = 1;// eslint-disable-line no-unused-vars\n", true],
  [
    "template-documentation.ts",
    'export const quoted = "/* oxlint-disable */";\nexport const example = `\n/* oxlint-disable */\n`;\n',
    false,
  ],
];

const RUST_CASES: readonly Case[] = [
  ["allow-dead-code.rs", "#[allow(dead_code)]\nfn hidden() {}\n", true],
  ["allow-dead-code-crate.rs", "#![allow(dead_code)]\nfn hidden() {}\n", true],
  ["cfg-allow-dead-code.rs", "#![cfg_attr(all(), allow(dead_code))]\nfn hidden() {}\n", true],
  ["multiline-allow-dead-code.rs", "#![\n  allow(\n    dead_code\n  )\n]\nfn hidden() {}\n", true],
  [
    "commented-allow-dead-code.rs",
    "#![allow(\n  /* policy */ dead_code\n)]\nfn hidden() {}\n",
    true,
  ],
  ["raw-ident-allow-dead-code.rs", "#[r#allow(r#dead_code)]\nfn hidden() {}\n", true],
  [
    "raw-ident-cfg-allow-dead-code.rs",
    "#![r#cfg_attr(all(), r#allow(r#dead_code))]\nfn hidden() {}\n",
    true,
  ],
  ["allow-unused-group.rs", "#[allow(unused)]\nfn hidden() {}\n", true],
  ["warn-dead-code.rs", "#[warn(dead_code)]\nfn hidden() {}\n", true],
  ["expect-dead-code.rs", "#[expect(dead_code)]\nfn hidden() {}\n", true],
  ["cfg-expect-unused-group.rs", "#![cfg_attr(all(), r#expect(r#unused))]\nfn hidden() {}\n", true],
  [
    "nested-bracket-cfg-allow-dead-code.rs",
    '#![cfg_attr(\n  all(),\n  doc = concat!["x"],\n  allow(dead_code)\n)]\nfn hidden() {}\n',
    true,
  ],
  [
    "char-before-allow-dead-code.rs",
    "const QUOTE: char = '\"';\nconst BYTE: u8 = b'\"';\nfn borrow<'a>(value: &'a str) -> &'a str { value }\n#[allow(dead_code)]\nfn hidden() {}\n",
    true,
  ],
  [
    "raw-string-documentation.rs",
    'pub const NORMAL: &str = "#[allow(dead_code)]";\n/*\n#[allow(dead_code)]\n*/\npub const EXAMPLE: &str = r#"\n#[allow(dead_code)]\n"#;\n',
    false,
  ],
];

const JSX_CASES: readonly Case[] = [
  [
    "jsx-documentation.tsx",
    "export const Example = () => <pre>/* oxlint-disable */</pre>;\nexport const Multiline = () => (\n  <pre>\n    /* oxlint-disable */\n  </pre>\n);\n",
    false,
  ],
  [
    "jsx-arrow-documentation.tsx",
    "export const Example = () => <pre>(value) => /* oxlint-disable */</pre>;\n",
    false,
  ],
  [
    "jsx-active-disable.tsx",
    "export const Example = () => <pre>{/* oxlint-disable */ null}</pre>;\n",
    true,
  ],
  [
    "tsx-generic-active-disable.tsx",
    "export const identity = <T,>(value: T) => value;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "tsx-const-generic-active-disable.tsx",
    "export const tuple = <const T,>(value: T) => value;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "tsx-default-generic-active-disable.tsx",
    "export const fallback = <T = unknown>(value: T) => value;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "jsx-regex-active-disable.tsx",
    'export const Example = () => (\n  <div>{/[}]/.test("}") /* oxlint-disable */}</div>\n);\nconst hidden = 1;\n',
    true,
  ],
  [
    "jsx-control-regex-active-disable.tsx",
    "export const Example = (ready: boolean, input: string) => <div>{(() => { if (ready) /}/.test(input); } /* oxlint-disable */)()}</div>;\nconst hidden = 1;\n",
    true,
  ],
  [
    "jsx-do-regex-active-disable.tsx",
    "export const Example = (input: string) => <div>{(() => { do /}/.test(input); while (false); } /* oxlint-disable */)()}</div>;\nconst hidden = 1;\n",
    true,
  ],
  [
    "jsx-else-regex-active-disable.tsx",
    "export const Example = (ready: boolean, input: string) => <div>{(() => { if (ready) input.trim(); else /}/.test(input); } /* oxlint-disable */)()}</div>;\nconst hidden = 1;\n",
    true,
  ],
  [
    "template-regex-active-disable.ts",
    'export const example = `${/\\}/.test("}") /* oxlint-disable */}`;\nconst hidden = 1;\n',
    true,
  ],
  [
    "jsx-division-active-disable.tsx",
    "export const Example = (total: number, divisor: number) => (\n  <div>{total / divisor /* oxlint-disable */}</div>\n);\n",
    true,
  ],
];

const DIVISION_CASES: readonly Case[] = [
  [
    "postfix-increment-active-disable.ts",
    "let counter = 1;\nexport const value = counter++ / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "postfix-decrement-active-disable.ts",
    "let counter = 1;\nexport const value = counter-- / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "non-null-division-active-disable.ts",
    "export const divide = (value: number | undefined, divisor: number) => value! / divisor /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "commented-postfix-division-active-disable.ts",
    "let counter = 1;\nexport const value = counter++ /* retain old value */ / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "commented-postfix-decrement-active-disable.ts",
    "let counter = 1;\nexport const value = counter-- /* retain old value */ / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "commented-non-null-division-active-disable.ts",
    "export const divide = (value: number | undefined) => value! /* checked */ / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "multiline-commented-postfix-division-active-disable.ts",
    "let counter = 1;\nexport const value = counter++ /* retain\nold value */ / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "regex-value-division-active-disable.ts",
    "export const ratio = /[}]/ /* completed regex */ / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "member-if-division-active-disable.ts",
    "const object = { if(value: number) { return value; } };\nexport const ratio = object.if(4) / 2 /* oxlint-disable */;\nconst hidden = 1;\n",
    true,
  ],
  [
    "private-if-division-active-disable.ts",
    "export class Counter {\n  #if(value: number) { return value; }\n  ratio(value: number) { return this.#if(value) / 2 /* oxlint-disable */; }\n}\nconst hidden = 1;\n",
    true,
  ],
];

const GENERIC_CASES: readonly Case[] = [
  [
    "tsx-generic-regex-active-disable.tsx",
    "export const matches = <T,>(\n  value: T,\n  pattern = /[)]/,\n) => pattern.test(String(value));\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "tsx-generic-comment-active-disable.tsx",
    "export const identity = <T,>(value: T /* ) */) => value;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "tsx-generic-boundary-comments-active-disable.tsx",
    "export const identity = <T,>/* type boundary */(value: T)/* value boundary */=> value;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "tsx-generic-control-regex-active-disable.tsx",
    "export const pair = <T,>(ready: boolean, input: string, value: T, test = (() => { if (ready) /)/.test(input); })) => [value, test] as const;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "tsx-generic-do-regex-active-disable.tsx",
    "export const pair = <T,>(input: string, value: T, test = (() => { do /)/.test(input); while (false); })) => [value, test] as const;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
  [
    "tsx-generic-else-regex-active-disable.tsx",
    "export const pair = <T,>(ready: boolean, input: string, value: T, test = (() => { if (ready) input.trim(); else /)/.test(input); })) => [value, test] as const;\nconst hidden = 1;// oxlint-disable-line no-unused-vars\n",
    true,
  ],
];

const CASES = [
  ...DIRECTIVE_CASES,
  ...RUST_CASES,
  ...JSX_CASES,
  ...DIVISION_CASES,
  ...GENERIC_CASES,
];

for (const [file, source, forbidden] of CASES) {
  test.concurrent(`AC-23 --file ${file}: ${forbidden ? "rejected" : "legal"}`, async () => {
    using tree = buildFixture("clean", { [`src/utilities/${file}`]: source });
    const res = await gr(tree.root, [
      "--only",
      "lint-suppressions",
      "--file",
      `src/utilities/${file}`,
    ]);
    expect([res.exit, count(res.out, "lint-suppressions")]).toEqual(forbidden ? [1, 1] : [0, 0]);
  });
}

test.concurrent("AC-23 repo mode finds all 42 active forms and flags no documentation form", async () => {
  const files = Object.fromEntries(
    CASES.map(([file, source]) => [`src/utilities/${file}`, source]),
  );
  using tree = buildFixture("clean", files);
  const { exit, out } = await gr(tree.root, ["--only", "lint-suppressions"]);
  expect(CASES.filter(([, , forbidden]) => forbidden)).toHaveLength(42);
  expect([exit, count(out, "lint-suppressions")]).toEqual([1, 42]);
  for (const [file, , forbidden] of CASES) {
    if (!forbidden) expect(out).not.toContain(file);
  }
});

test.concurrent("AC-23 the violating fixture's no-unused-vars directive fails repo and --file modes", async () => {
  using tree = buildFixture("violating");
  const repo = await gr(tree.root, []);
  expect(count(repo.out, "lint-suppressions")).toBe(1);
  expect(repo.out).toContain("suppressed-unused.ts");
  const file = await gr(tree.root, ["--file", "src/utilities/suppressed-unused.ts"]);
  expect([file.exit, count(file.out, "lint-suppressions")]).toEqual([1, 1]);
});

test.concurrent("repo mode skips binary files and the pruned top-level trees", async () => {
  using tree = buildFixture("clean", {
    "src/utilities/binary.ts": "\0/* oxlint-disable */\n",
    "dist/built.js": "/* oxlint-disable */\n",
    "node_modules/pkg/index.js": "/* oxlint-disable */\n",
  });
  expect(await gr(tree.root, ["--only", "lint-suppressions"])).toEqual({ exit: 0, out: "" });
});
