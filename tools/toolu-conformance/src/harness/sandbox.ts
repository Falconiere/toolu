/**
 * Disposable per-test sandbox (#251): a temp project, HOME and host config
 * roots owned by one value. `using sb = createSandbox()` removes the tree when
 * the test's scope ends, even when it throws — no module state, so any number
 * of tests can run concurrently.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";

export type HostName = "claude" | "codex" | "cursor" | "opencode";
export type ConfigScope = "project" | "user";

export type SandboxOptions = {
  /** Initialise `project` as a git repo with one commit. */
  git?: boolean;
  /** Initial branch when `git` is set. Default `main`. */
  branch?: string;
  /** Files written into `project` before the initial commit (relative path → body). */
  files?: Record<string, string>;
};

export type Sandbox = {
  readonly root: string;
  readonly project: string;
  readonly home: string;
  readonly codexHome: string;
  path(rel: string): string;
  write(rel: string, body: string | object): string;
  read(rel: string): string;
  git(...args: string[]): string;
  configDir(host: HostName, scope: ConfigScope): string;
  writeConfig(host: HostName, scope: ConfigScope, config: object): string;
  [Symbol.dispose](): void;
};

const PROJECT_CONFIG_DIR: Record<HostName, string> = {
  claude: ".claude",
  codex: ".codex",
  cursor: ".cursor",
  opencode: ".opencode",
};

/** Resolve `rel` under `base`, refusing anything that escapes it. */
function inside(base: string, rel: string): string {
  const abs = resolve(base, rel);
  const back = relative(base, abs);
  if (back.startsWith("..") || isAbsolute(back)) {
    throw new Error(`sandbox path escapes ${base}: ${rel}`);
  }
  return abs;
}

function writeAt(abs: string, body: string | object): string {
  mkdirSync(dirname(abs), { recursive: true });
  const text = typeof body === "string" ? body : `${JSON.stringify(body, null, 2)}\n`;
  writeFileSync(abs, text, "utf8");
  return abs;
}

function gitIn(project: string, args: string[]): string {
  const res = spawnSync("git", ["-C", project, ...args], { encoding: "utf8" });
  if (res.error) {
    throw new Error(`git ${args.join(" ")}: ${res.error.message}`);
  }
  if (res.status !== 0) {
    throw new Error(`git ${args.join(" ")} exited ${String(res.status)}: ${res.stderr.trim()}`);
  }
  return res.stdout;
}

function initRepo(project: string, branch: string, files: Record<string, string>): void {
  gitIn(project, ["init", "-q", "-b", branch]);
  gitIn(project, ["config", "user.email", "harness@toolu.test"]);
  gitIn(project, ["config", "user.name", "toolu harness"]);
  gitIn(project, ["config", "commit.gpgsign", "false"]);
  if (Object.keys(files).length === 0) {
    writeAt(join(project, ".gitkeep"), "");
  }
  gitIn(project, ["add", "-A"]);
  gitIn(project, ["commit", "-q", "-m", "harness: initial commit"]);
}

type Roots = { project: string; home: string; codexHome: string };

const USER_CONFIG_DIR: Record<HostName, (roots: Roots) => string> = {
  claude: (roots) => join(roots.home, ".claude"),
  codex: (roots) => roots.codexHome,
  cursor: (roots) => join(roots.home, ".cursor"),
  opencode: (roots) => join(roots.home, ".config", "opencode"),
};

function configDirFor(roots: Roots, host: HostName, scope: ConfigScope): string {
  return scope === "project"
    ? join(roots.project, PROJECT_CONFIG_DIR[host])
    : USER_CONFIG_DIR[host](roots);
}

/** Create a sandbox; the caller owns it (`using sb = createSandbox()`). */
export function createSandbox(opts: SandboxOptions = {}): Sandbox {
  // realpath: macOS tmpdir is a /var -> /private/var symlink, and hooks that
  // resolve their cwd would otherwise disagree with the paths a test asserts on.
  const root = realpathSync(mkdtempSync(join(tmpdir(), "toolu-harness-")));
  const project = join(root, "project");
  const home = join(root, "home");
  const codexHome = join(home, ".codex");
  mkdirSync(project, { recursive: true });
  mkdirSync(codexHome, { recursive: true });
  const files = opts.files ?? {};
  for (const [rel, body] of Object.entries(files)) {
    writeAt(inside(project, rel), body);
  }
  if (opts.git === true) {
    initRepo(project, opts.branch ?? "main", files);
  }
  const dirs = { project, home, codexHome };
  return {
    root,
    project,
    home,
    codexHome,
    path: (rel) => inside(project, rel),
    write: (rel, body) => writeAt(inside(project, rel), body),
    read: (rel) => readFileSync(inside(project, rel), "utf8"),
    git: (...args) => gitIn(project, args),
    configDir: (host, scope) => configDirFor(dirs, host, scope),
    writeConfig: (host, scope, config) =>
      writeAt(join(configDirFor(dirs, host, scope), "toolu.config.json"), config),
    [Symbol.dispose]: () => rmSync(root, { recursive: true, force: true }),
  };
}
