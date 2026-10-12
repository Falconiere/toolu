/** Live macOS smoke of the native toolu hooks in Claude Code and Codex sessions. */
import { spawn, spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../..");
const BIN = process.env.TOOLU_BIN ?? join(ROOT, "target/debug/toolu");
const PROMPT_DENY =
  "Use the shell tool exactly once to run printf changed > .env in this temporary project. Report whether the hook blocks it. Do not retry or use another tool.";
const PROMPT_COMMIT =
  'Use the shell tool once to run git add sample.ts && git commit -m "test: quality gate". Report the hook result. Do not retry or bypass hooks.';
const PROMPT_CODEX_COMMIT =
  'Use the shell tool once to run git commit -m "test: quality gate". Report the hook result. Do not retry or bypass hooks.';
const LIMITS = {
  gates: {
    protectedFiles: { mode: "block" },
    commitGate: { mode: "block" },
    qualityGate: { mode: "block" },
  },
  lang: { ts: { maxFileLines: 3 } },
};

function run(argv: string[], cwd: string, env: NodeJS.ProcessEnv, log?: string): string {
  const command = argv[0];
  if (!command) throw new Error("run: empty argv");
  const result = spawnSync(command, argv.slice(1), {
    cwd,
    env,
    encoding: "utf8",
    timeout: 180_000,
  });
  const output = `${result.stdout}${result.stderr}`;
  if (log !== undefined) writeFileSync(log, output);
  if (result.error || result.status !== 0) {
    throw new Error(
      `${command} exited ${String(result.status)}: ${result.error?.message ?? output.slice(-800)}`,
    );
  }
  return output;
}

function must(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

function project(root: string, host: "claude" | "codex"): string {
  const path = join(root, `${host} project with spaces`);
  mkdirSync(join(path, `.${host}`), { recursive: true });
  writeFileSync(join(path, ".env"), "KEEP=1\n");
  writeFileSync(join(path, "sample.ts"), "export const n = 1;\n");
  writeFileSync(join(path, "tsconfig.json"), "{}\n");
  writeFileSync(join(path, "package.json"), '{"name":"native-host-smoke","private":true}\n');
  writeFileSync(join(path, "bun.lock"), "");
  writeFileSync(join(path, `.${host}/toolu.config.json`), `${JSON.stringify(LIMITS)}\n`);
  const env = { ...process.env, GIT_CONFIG_NOSYSTEM: "1" };
  for (const args of [
    ["init", "-q"],
    ["config", "user.name", "Toolu Smoke"],
    ["config", "user.email", "smoke@toolu.test"],
    ["add", "-A"],
    ["commit", "-qm", "fixture: initial"],
  ])
    run(["git", ...args], path, env);
  return path;
}

function head(path: string, env: NodeJS.ProcessEnv): string {
  return run(["git", "rev-parse", "HEAD"], path, env).trim();
}

function checkDeny(path: string, transcript: string): void {
  must(readFileSync(join(path, ".env"), "utf8") === "KEEP=1\n", "protected .env changed");
  must(transcript.includes("PreToolUse") && transcript.includes(".env"), "no live deny evidence");
}

function checkQuality(path: string, host: "claude" | "codex"): void {
  const gate = join(path, `.${host}`, "tmp/quality-gate-status.json");
  must(existsSync(gate), `quality gate file missing: ${gate}`);
  const state = readFileSync(gate, "utf8");
  must(
    state.includes('"status": "failing"') && state.includes("TS file exceeds 3-line limit"),
    "quality failure not recorded",
  );
  must(
    readFileSync(join(path, "sample.ts"), "utf8").split("\n").filter(Boolean).length >= 8,
    "source edit missing",
  );
}

function claude(root: string, path: string): void {
  const env = { ...process.env, TOOLU_BIN: BIN };
  const base = [
    "claude",
    "--plugin-dir",
    join(ROOT, "plugins/toolu"),
    "--plugin-dir",
    join(ROOT, "plugins/ts-quality"),
    "--model",
    "haiku",
    "-p",
    "--permission-mode",
    "auto",
    "--output-format",
    "stream-json",
    "--verbose",
  ];
  const args = (tools: string, prompt: string): string[] => [
    ...base,
    "--tools",
    tools,
    "--max-budget-usd",
    "2",
    prompt,
  ];
  const deny = run(args("Bash", PROMPT_DENY), path, env, join(root, "claude-deny.jsonl"));
  checkDeny(path, deny);
  const allowed = run(
    args("Bash", "Use Bash once to run pwd, then stop."),
    path,
    env,
    join(root, "claude-allowed.jsonl"),
  );
  must(allowed.includes(path), "Claude clean Bash control did not run");
  const edit = run(
    args(
      "Read,Edit",
      "Use Read on sample.ts, then Edit to replace its one line with eight nonblank export const statements. The configured file limit is three code lines. Stop after the edit; do not fix it.",
    ),
    path,
    env,
    join(root, "claude-edit.jsonl"),
  );
  must(edit.includes('"name":"Edit"'), "Claude did not use Edit");
  checkQuality(path, "claude");
  const before = head(path, env);
  const commit = run(args("Bash", PROMPT_COMMIT), path, env, join(root, "claude-commit.jsonl"));
  must(commit.includes("quality gate failing"), "Claude commit was not denied by quality gate");
  must(head(path, env) === before, "Claude commit changed HEAD");
  process.stdout.write("claude: native deny, post-edit quality state and commit gate passed\n");
}

async function codexEdit(
  argv: string[],
  path: string,
  env: NodeJS.ProcessEnv,
  log: string,
): Promise<void> {
  const command = argv[0];
  if (!command) throw new Error("codexEdit: empty argv");
  const gate = join(path, ".codex/tmp/quality-gate-status.json");
  const source = join(path, "sample.ts");
  let output = "";
  let failingGate: string | undefined;
  let failingSource: string | undefined;
  const child = spawn(command, argv.slice(1), { cwd: path, env, timeout: 180_000 });
  child.stdin.end();
  child.stdout.on("data", (chunk: Buffer) => {
    output += chunk.toString();
  });
  child.stderr.on("data", (chunk: Buffer) => {
    output += chunk.toString();
  });
  const poll = setInterval(() => {
    if (failingGate !== undefined || !existsSync(gate)) return;
    const state = readFileSync(gate, "utf8");
    if (!state.includes('"status": "failing"')) return;
    failingGate = state;
    failingSource = readFileSync(source, "utf8");
  }, 20);
  const status = await new Promise<number | null>((finish, reject) => {
    child.once("error", reject);
    child.once("close", finish);
  }).finally(() => clearInterval(poll));
  writeFileSync(log, output);
  if (status !== 0 || failingGate === undefined || failingSource === undefined) {
    throw new Error("Codex did not record a live failing edit");
  }
  // Preserve the observed live failure if the model repairs it in the same turn.
  writeFileSync(source, failingSource);
  writeFileSync(gate, failingGate);
}

async function codex(root: string, path: string): Promise<void> {
  const home = join(root, "codex-home");
  mkdirSync(home, { mode: 0o700 });
  const auth = join(homedir(), ".codex/auth.json");
  if (existsSync(auth)) symlinkSync(auth, join(home, "auth.json"));
  const env = { ...process.env, CODEX_HOME: home, TOOLU_BIN: BIN };
  run(["codex", "plugin", "marketplace", "add", ROOT], path, env);
  for (const name of ["toolu", "ts-quality"]) {
    run(["codex", "plugin", "add", `${name}@toolu`], path, env);
  }
  // This private test profile installs only the two reviewed checkout plugins.
  const base = [
    "codex",
    "--ask-for-approval",
    "never",
    "--sandbox",
    "workspace-write",
    "--dangerously-bypass-hook-trust",
    "exec",
    "--json",
    "--ephemeral",
    "-C",
    path,
  ];
  const deny = run([...base, PROMPT_DENY], path, env, join(root, "codex-deny.jsonl"));
  checkDeny(path, deny);
  const allowed = run(
    [...base, "Use the shell tool once to run pwd, then stop."],
    path,
    env,
    join(root, "codex-allowed.jsonl"),
  );
  must(allowed.includes(path), "Codex clean shell control did not run");
  await codexEdit(
    [
      ...base,
      "Use the apply_patch tool to replace the one line in sample.ts with eight nonblank export const statements. This is a quality-gate test; stop after the edit and report the hook result.",
    ],
    path,
    env,
    join(root, "codex-edit.jsonl"),
  );
  checkQuality(path, "codex");
  run(["git", "add", "sample.ts"], path, env);
  const before = head(path, env);
  const commit = run([...base, PROMPT_CODEX_COMMIT], path, env, join(root, "codex-commit.jsonl"));
  must(commit.includes("quality gate failing"), "Codex commit was not denied by quality gate");
  must(head(path, env) === before, "Codex commit changed HEAD");
  process.stdout.write("codex: native deny, post-edit quality state and commit gate passed\n");
}

async function main(): Promise<void> {
  must(process.platform === "darwin", "live host smoke requires macOS");
  must(existsSync(BIN), `native binary missing: ${BIN}`);
  const root = mkdtempSync(join(tmpdir(), "toolu-native-host-"));
  chmodSync(root, 0o700);
  let passed = false;
  try {
    claude(root, project(root, "claude"));
    await codex(root, project(root, "codex"));
    passed = true;
    process.stdout.write("native host smoke: ok\n");
  } catch (error) {
    process.stderr.write(`native host smoke evidence: ${root}\n`);
    throw error;
  } finally {
    const auth = join(root, "codex-home/auth.json");
    if (existsSync(auth)) rmSync(auth);
    if (passed && process.env.TOOLU_HOST_SMOKE_KEEP !== "1")
      rmSync(root, { recursive: true, force: true });
  }
}

await main();
