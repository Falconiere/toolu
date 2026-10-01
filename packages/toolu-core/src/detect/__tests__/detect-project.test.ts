/** Project detection against real git repositories and marker files. */
import { expect, test } from "bun:test";
import { mkdirSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import {
  detectClippy,
  detectPython,
  detectRust,
  detectTs,
  nodePackageManager,
  projectName,
  projectToplevel,
  pythonLinter,
  toRelativePath,
  tsLinter,
  type DetectOptions,
} from "../detect-project.ts";
import { detectEnv } from "./detect-env.ts";

interface Layout {
  readonly name: string;
  /** Files committed to the repository. */
  readonly tracked?: Record<string, string>;
  /** Files written but never added. */
  readonly untracked?: Record<string, string>;
  /** Directories created at the root. */
  readonly dirs?: readonly string[];
  /** Symlinks at the root, name → target. */
  readonly links?: Record<string, string>;
}

const LAYOUTS: readonly Layout[] = [
  { name: "empty repository" },
  { name: "bun.lock", tracked: { "bun.lock": "" } },
  { name: "bun.lockb", tracked: { "bun.lockb": "" } },
  { name: "pnpm-lock.yaml", tracked: { "pnpm-lock.yaml": "" } },
  { name: "yarn.lock", tracked: { "yarn.lock": "" } },
  { name: "package-lock.json", tracked: { "package-lock.json": "{}" } },
  {
    name: "lock precedence",
    tracked: { "yarn.lock": "", "package-lock.json": "{}", "pnpm-lock.yaml": "" },
  },
  { name: "Cargo.toml", tracked: { "Cargo.toml": "[package]\n" } },
  { name: "Cargo.toml is a directory", dirs: ["Cargo.toml"] },
  { name: "pyproject.toml", tracked: { "pyproject.toml": "[project]\n" } },
  { name: "setup.py", tracked: { "setup.py": "" } },
  { name: "setup.cfg", tracked: { "setup.cfg": "" } },
  { name: "requirements.txt", tracked: { "requirements.txt": "" } },
  { name: "tsconfig.json tracked", tracked: { "tsconfig.json": "{}" } },
  { name: "tsconfig untracked", untracked: { "tsconfig.json": "{}" } },
  { name: "nested tsconfig", tracked: { "packages/a/tsconfig.base.json": "{}" } },
  { name: "pathspec * crosses /", tracked: { "tsconfig-x/foo.json": "{}" } },
  { name: "tsconfig in name only", tracked: { "src/my-tsconfig.json": "{}" } },
  {
    name: "biome wins",
    tracked: { "biome.json": "{}", ".oxlintrc.json": "{}", ".eslintrc.cjs": "" },
  },
  { name: "biome.jsonc", tracked: { "biome.jsonc": "{}" } },
  { name: "oxc over eslint", tracked: { ".oxlintrc.json": "{}", "eslint.config.mjs": "" } },
  { name: "legacy .eslintrc.cjs", tracked: { ".eslintrc.cjs": "" } },
  { name: "bare .eslintrc", tracked: { ".eslintrc": "{}" } },
  { name: "eslint.config.js", tracked: { "eslint.config.js": "" } },
  { name: ".eslintrc directory", dirs: [".eslintrc.d"] },
  { name: "ruff.toml", tracked: { "ruff.toml": "" } },
  { name: ".ruff.toml", tracked: { ".ruff.toml": "" } },
  { name: "[tool.ruff] in pyproject", tracked: { "pyproject.toml": "[project]\n[tool.ruff]\n" } },
  {
    name: "[tool.ruff.lint] in pyproject",
    tracked: { "pyproject.toml": "[tool.ruff.lint]\nselect = []" },
  },
  { name: "indented [tool.ruff]", tracked: { "pyproject.toml": "  [tool.ruff]\n" } },
  { name: "CR before [tool.ruff]", tracked: { "pyproject.toml": "x = 1\r[tool.ruff]\n" } },
  { name: "clippy.toml", tracked: { "clippy.toml": "" } },
  { name: ".clippy.toml", tracked: { ".clippy.toml": "" } },
  {
    name: "symlinked Cargo.toml",
    tracked: { "real.toml": "" },
    links: { "Cargo.toml": "real.toml" },
  },
  { name: "dangling symlink marker", links: { "setup.py": "missing.py" } },
];

/** Project probes in a stable order for fixture assertions. */
function tsProbe(o: DetectOptions): string[] {
  const flag = (on: boolean, word: string) => (on ? word : "");
  return [
    projectToplevel(o) ?? "",
    projectName(o) ?? "",
    nodePackageManager(o) ?? "",
    flag(detectRust(o), "rust"),
    flag(detectPython(o), "python"),
    flag(detectTs(o), "ts"),
    tsLinter(o) ?? "",
    pythonLinter(o) ?? "",
    flag(detectClippy(o), "clippy"),
  ];
}

function build(sb: Sandbox, layout: Layout): void {
  for (const [rel, body] of Object.entries(layout.tracked ?? {})) sb.write(rel, body);
  for (const dir of layout.dirs ?? []) mkdirSync(sb.path(dir), { recursive: true });
  for (const [name, target] of Object.entries(layout.links ?? {})) {
    symlinkSync(target, sb.path(name));
  }
  sb.git("add", "-A");
  sb.git("commit", "-q", "--allow-empty", "-m", "markers");
  for (const [rel, body] of Object.entries(layout.untracked ?? {})) sb.write(rel, body);
}

test.concurrent.each(LAYOUTS.map((layout) => [layout.name, layout] as const))(
  "%s: project probes are stable from the root and a subdirectory",
  (_name, layout) => {
    using sb = createSandbox({ git: true });
    build(sb, layout);
    const env = detectEnv(sb.home);
    const sub = sb.path("sub/dir");
    mkdirSync(sub, { recursive: true });
    const root = tsProbe({ env, cwd: sb.project });
    expect(root[0]).toBe(sb.project);
    expect(root[1]).toBe("project");
    expect(tsProbe({ env, cwd: sub })).toEqual(root);
  },
  60_000,
);

const BATS_ANSWERS: Readonly<Record<string, readonly [number, string]>> = {
  "bun.lock": [2, "bun"],
  "pnpm-lock.yaml": [2, "pnpm"],
  "package-lock.json": [2, "npm"],
  "lock precedence": [2, "pnpm"],
  "Cargo.toml": [3, "rust"],
  "Cargo.toml is a directory": [3, ""],
  "setup.cfg": [4, "python"],
  "tsconfig.json tracked": [5, "ts"],
  "tsconfig untracked": [5, ""],
  "pathspec * crosses /": [5, "ts"],
  "biome wins": [6, "biome"],
  "oxc over eslint": [6, "oxc"],
  "legacy .eslintrc.cjs": [6, "eslint"],
  "[tool.ruff] in pyproject": [7, "ruff"],
  "indented [tool.ruff]": [7, ""],
  ".clippy.toml": [8, "clippy"],
};

test.concurrent.each(Object.entries(BATS_ANSWERS))(
  "%s gives the answer bats asserts",
  (name, [index, answer]) => {
    using sb = createSandbox({ git: true });
    build(sb, LAYOUTS.find((l) => l.name === name) ?? { name });
    const probe = tsProbe({ env: detectEnv(sb.home), cwd: sb.project });
    expect(probe[index]).toBe(answer);
    expect(probe[1]).toBe("project");
  },
);

test("outside a repository every probe is empty", () => {
  using sb = createSandbox();
  sb.write("Cargo.toml", "");
  sb.write("tsconfig.json", "{}");
  const env = detectEnv(sb.home);
  expect(tsProbe({ env, cwd: sb.project }).join("")).toBe("");
});

test("toRelativePath resolves paths inside and outside a repository", () => {
  using sb = createSandbox({ git: true });
  const outside = join(sb.root, "outside");
  mkdirSync(outside);
  const env = detectEnv(sb.home);
  const inputs = [
    sb.path("src/a.ts"),
    join(sb.project, "deep/x/y.rs"),
    "src/a.ts",
    "/etc/hosts",
    "",
    sb.project,
    `${sb.project}/`,
    `${sb.project}-sibling/a.ts`,
    "a path with spaces/x.ts",
  ];
  for (const cwd of [sb.project, outside]) {
    const result = inputs.map((p) => toRelativePath(p, { env, cwd }));
    expect(result).toHaveLength(inputs.length);
    expect(result[0]).toBe(cwd === outside ? sb.path("src/a.ts") : "src/a.ts");
  }
  expect(toRelativePath(sb.path("src/a.ts"), { env, cwd: sb.project })).toBe("src/a.ts");
});
