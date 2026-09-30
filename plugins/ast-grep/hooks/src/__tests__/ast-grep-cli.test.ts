/**
 * AC-5 (#268): the skill wrapper bundle, run against the real ast-grep CLI on
 * real fixture files, prints what the CLI prints when invoked with the argv
 * the bash wrapper built. The live CLI is the oracle, not a golden, because
 * ast-grep's output changes across versions. PATH holds only links to the real
 * binary, named `sg` and/or `ast-grep`, so a system `sg` (shadow-utils on
 * Linux) never answers.
 */
import { expect, test } from "bun:test";
import { mkdirSync, realpathSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type RunResult } from "@toolu/conformance/harness/spawn";
import { PLUGIN_ROOT } from "./golden-sandbox.ts";

const WRAPPER = join(PLUGIN_ROOT, "hooks/dist/ast-grep.js");
const found = Bun.which("ast-grep") ?? Bun.which("sg");
if (found === null) throw new Error("ast-grep-cli.test: the real ast-grep CLI must be on PATH");
const REAL = realpathSync(found);

const PATTERN = "console.log($A)";
const RULE = [
  "id: no-console",
  "language: typescript",
  "severity: warning",
  "message: no console",
  "rule:",
  "  pattern: console.log($A)",
].join("\n");

/** A project with TypeScript, TSX and plain-text files, and PATH dirs offering `names`. */
function project(sb: Sandbox): { bin: (names: readonly string[]) => string } {
  sb.write("src/a.ts", 'export function f(): void {\n  console.log("a");\n}\n');
  sb.write("src/b.tsx", 'export const B = () => { console.log("b"); return <p />; };\n');
  sb.write("src/notes.txt", 'console.log("text")\n');
  sb.write("rule.yml", `${RULE}\n`);
  return {
    bin: (names) => {
      const dir = sb.path(`bin-${names.join("-") || "none"}`);
      mkdirSync(dir, { recursive: true });
      for (const name of names) symlinkSync(REAL, join(dir, name));
      return dir;
    },
  };
}

type Outcome = Pick<RunResult, "stdout" | "stderr" | "exitCode">;

async function outcome(argv: string[], sb: Sandbox, path: string): Promise<Outcome> {
  const { stdout, stderr, exitCode } = await run(argv, { cwd: sb.project, env: { PATH: path } });
  return { stdout, stderr, exitCode };
}

function wrapper(sb: Sandbox, path: string, args: readonly string[]): Promise<Outcome> {
  return outcome([process.execPath, WRAPPER, ...args], sb, path);
}

function cli(sb: Sandbox, binary: string, args: readonly string[]): Promise<Outcome> {
  return outcome([binary, ...args], sb, "");
}

type Case = { name: string; args: string[]; oracle: string[] };

const CASES: Case[] = [
  {
    name: "search infers --lang from a .ts path",
    args: ["search", PATTERN, "src/a.ts"],
    oracle: ["run", "--pattern", PATTERN, "--color", "never", "--lang", "typescript", "src/a.ts"],
  },
  {
    name: "search infers tsx from a .tsx path",
    args: ["search", PATTERN, "src/b.tsx"],
    oracle: ["run", "--pattern", PATTERN, "--color", "never", "--lang", "tsx", "src/b.tsx"],
  },
  {
    name: "search keeps an explicit -l",
    args: ["search", PATTERN, "-l", "typescript", "src/a.ts"],
    oracle: ["run", "--pattern", PATTERN, "--color", "never", "-l", "typescript", "src/a.ts"],
  },
  {
    name: "search infers nothing from a directory or an unknown extension",
    args: ["search", PATTERN, "--lang=typescript", "src", "src/notes.txt"],
    oracle: [
      "run",
      "--pattern",
      PATTERN,
      "--color",
      "never",
      "--lang=typescript",
      "src",
      "src/notes.txt",
    ],
  },
  {
    name: "files lists matching paths",
    args: ["files", PATTERN, "src/a.ts"],
    oracle: [
      "run",
      "--pattern",
      PATTERN,
      "--files-with-matches",
      "--color",
      "never",
      "--lang",
      "typescript",
      "src/a.ts",
    ],
  },
  {
    name: "scan with inline YAML",
    args: ["scan", RULE, "src"],
    oracle: [
      "scan",
      "--inline-rules",
      RULE,
      "--report-style",
      "short",
      "--max-results",
      "50",
      "--color",
      "never",
      "src",
    ],
  },
  {
    name: "scan with a rule file",
    args: ["scan", "rule.yml", "src"],
    oracle: [
      "scan",
      "--rule",
      "rule.yml",
      "--report-style",
      "short",
      "--max-results",
      "50",
      "--color",
      "never",
      "src",
    ],
  },
  {
    name: "debug prints the pattern tree",
    args: ["debug", PATTERN, "src/a.ts"],
    oracle: [
      "run",
      "--pattern",
      PATTERN,
      "--debug-query=pattern",
      "--color",
      "never",
      "--lang",
      "typescript",
      "src/a.ts",
    ],
  },
];

for (const c of CASES) {
  test.concurrent(`${c.name}: same output as the CLI given the bash wrapper's argv`, async () => {
    using sb = createSandbox();
    const bin = project(sb).bin(["sg"]);
    const [actual, expected] = await Promise.all([
      wrapper(sb, bin, c.args),
      cli(sb, join(bin, "sg"), c.oracle),
    ]);
    expect(actual).toEqual(expected);
    expect(`${expected.stdout}${expected.stderr}`).not.toBe("");
  });
}

test.concurrent("an extension naming an Object.prototype member infers nothing", async () => {
  using sb = createSandbox();
  const bin = project(sb).bin(["sg"]);
  sb.write("src/x.constructor", 'console.log("x");\n');
  const [actual, expected] = await Promise.all([
    wrapper(sb, bin, ["search", PATTERN, "src/x.constructor"]),
    cli(sb, join(bin, "sg"), [
      "run",
      "--pattern",
      PATTERN,
      "--color",
      "never",
      "src/x.constructor",
    ]),
  ]);
  expect(actual).toEqual(expected);
});

// `--lang tsx` would skip the .ts path and print nothing.
test.concurrent("a pattern naming an existing file never feeds --lang inference", async () => {
  using sb = createSandbox();
  const bin = project(sb).bin(["sg"]);
  sb.write("src/c.ts", "export const c = row.tsx;\n");
  sb.write("$A.tsx", "export const d = 1;\n");
  const [actual, expected] = await Promise.all([
    wrapper(sb, bin, ["search", "$A.tsx", "src/c.ts"]),
    cli(sb, join(bin, "sg"), [
      "run",
      "--pattern",
      "$A.tsx",
      "--color",
      "never",
      "--lang",
      "typescript",
      "src/c.ts",
    ]),
  ]);
  expect(actual).toEqual(expected);
  expect(expected.stdout).toContain("row.tsx");
});

test.concurrent("falls back to ast-grep when sg is absent", async () => {
  using sb = createSandbox();
  const bin = project(sb).bin(["ast-grep"]);
  const args = [
    "run",
    "--pattern",
    PATTERN,
    "--color",
    "never",
    "--lang",
    "typescript",
    "src/a.ts",
  ];
  const [actual, expected] = await Promise.all([
    wrapper(sb, bin, ["search", PATTERN, "src/a.ts"]),
    cli(sb, join(bin, "ast-grep"), args),
  ]);
  expect(actual).toEqual(expected);
  expect(actual.stdout).toContain('console.log("a")');
});

test.concurrent("with neither binary on PATH it exits 0 silently", async () => {
  using sb = createSandbox();
  const bin = project(sb).bin([]);
  expect(await wrapper(sb, bin, ["search", PATTERN, "src/a.ts"])).toMatchObject({
    exitCode: 0,
    stdout: "",
    stderr: "",
  });
});

for (const args of [[], ["replace", PATTERN]]) {
  test.concurrent(`subcommand ${JSON.stringify(args[0] ?? "")} prints usage and exits 1`, async () => {
    using sb = createSandbox();
    const res = await wrapper(sb, project(sb).bin(["sg"]), args);
    expect(res.exitCode).toBe(1);
    expect(res.stdout).toStartWith("Usage: ast-grep.js <subcommand> [args...]\n\nSubcommands:\n");
    expect(res.stdout).toContain(
      "Pass-through flags: --globs <pat>, -A/-B/-C <N>, --max-results N\n",
    );
    expect(res.stderr).toBe("");
  });
}

for (const sub of ["search", "files", "debug", "scan"]) {
  test.concurrent(`${sub} without a pattern exits 1 with a message`, async () => {
    using sb = createSandbox();
    const res = await wrapper(sb, project(sb).bin(["sg"]), [sub]);
    expect(res).toMatchObject({ exitCode: 1, stdout: "" });
    expect(res.stderr).toStartWith(`ast-grep.js: ${sub} requires `);
  });
}
