/**
 * The detect layer shells out to git and nothing else (#254 AC-8). A child
 * `bun` process answers every detect question twice, once with the full PATH
 * and once with a PATH holding only `git` (plus the `sg` that
 * `detectAstGrep` looks for), and the answers must be the same.
 * `wc`, `grep`, `awk` and `jq`, which detect.sh needs, are unreachable in the
 * second run.
 */
import { expect, test } from "bun:test";
import { mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";

const DETECT = join(import.meta.dir, "../detect.ts");
const SHELL = join(import.meta.dir, "../../shell/shell.ts");

const PROBE = `import * as d from ${JSON.stringify(DETECT)};
import { analyzeShell } from ${JSON.stringify(SHELL)};
const [cwd = "", wt = "", file = ""] = process.argv.slice(2);
const o = { cwd };
const push = analyzeShell(\`git -C "\${wt}" push origin HEAD:feat/y\`);
console.log(JSON.stringify({
  toplevel: d.projectToplevel(o), name: d.projectName(o), pm: d.nodePackageManager(o),
  rust: d.detectRust(o), python: d.detectPython(o), ts: d.detectTs(o), tsLinter: d.tsLinter(o),
  pyLinter: d.pythonLinter(o), clippy: d.detectClippy(o), rel: d.toRelativePath(file, o),
  base: d.baseBranch(undefined, process.env, cwd), slug: d.branchSlug("feat/x"),
  astGrep: d.detectAstGrep(), lines: d.countCodeLines(file), py: d.countPythonCodeLines(file),
  open: d.hasUnterminatedBlock(file), isPush: d.isGitPush(push), isCommit: d.isGitCommit(push),
  root: d.pushTargetRoot(push, o), branch: d.pushTargetBranch(push, wt),
}));
`;

test("every detect answer is the same with only git on PATH", async () => {
  using sb = createSandbox({
    git: true,
    files: {
      "tsconfig.json": "{}",
      "Cargo.toml": "[package]\n",
      "bun.lock": "",
      "pyproject.toml": "[tool.ruff]\n",
      "biome.json": "{}",
      "clippy.toml": "",
      "src/a.ts": "/* doc */\nconst a = 1; // c\n\nexport { a };\n",
    },
  });
  const wt = sb.path("wt");
  sb.git("worktree", "add", "-q", "--detach", wt);
  const gitOnly = join(sb.root, "git-only");
  mkdirSync(gitOnly);
  symlinkSync(Bun.which("git") ?? "/usr/bin/git", join(gitOnly, "git"));
  // The tool `detectAstGrep` asks about, not a helper it runs.
  symlinkSync(Bun.which("true") ?? "/usr/bin/true", join(gitOnly, "sg"));
  const probe = join(sb.root, "probe.ts");
  writeFileSync(probe, PROBE);

  const answer = async (path: string) => {
    const res = await run([process.execPath, probe, sb.project, wt, sb.path("src/a.ts")], {
      cwd: sb.project,
      env: { HOME: sb.home, PATH: path },
    });
    expect(res.stderr).toBe("");
    expect(res.exitCode).toBe(0);
    return res.stdout;
  };
  const full = await answer(`${gitOnly}:${process.env.PATH ?? ""}`);
  const only = await answer(gitOnly);
  expect(only).toBe(full);
  expect(JSON.parse(only)).toMatchObject({
    name: "project",
    pm: "bun",
    rust: true,
    python: true,
    ts: true,
    tsLinter: "biome",
    pyLinter: "ruff",
    clippy: true,
    rel: "src/a.ts",
    base: "main",
    astGrep: true,
    lines: 2,
    open: false,
    isPush: true,
    isCommit: false,
    root: wt,
    branch: "feat/y",
  });
}, 60_000);
