/** One-time bash-vs-Bun parity for #274, run before the bash scripts are deleted. */
import { expect, test } from "bun:test";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";

const PLUGIN = resolve(import.meta.dir, "../../..");
const JEV_HOOK = resolve(PLUGIN, "../jev/hooks/dist/session-start.js");
const JEV_SH = resolve(PLUGIN, "../jev/skills/jev/scripts/jev.sh");
const RENDER = join(PLUGIN, "hooks/dist/statusline.js");

type Fixture = { cwd: string; payload: string; env: EnvPatch };
type Setup = (sb: Sandbox, cfg: string) => Partial<Fixture> | undefined;

function put(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

function git(cwd: string, ...args: string[]): void {
  const res = Bun.spawnSync(
    ["git", "-C", cwd, "-c", "user.email=t@t", "-c", "user.name=t", ...args],
    {
      stdout: "ignore",
      stderr: "pipe",
    },
  );
  if (res.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr.toString()}`);
}

function repo(dir: string): string {
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "-q", "-b", "main");
  git(dir, "commit", "-q", "--allow-empty", "-m", "init");
  return dir;
}

function withRemote(sb: Sandbox, branch = "main"): string {
  const remote = join(sb.root, "remote.git");
  const dir = join(sb.root, "repo");
  mkdirSync(remote, { recursive: true });
  git(remote, "init", "-q", "--bare");
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "-q", "-b", branch);
  git(dir, "commit", "-q", "--allow-empty", "-m", "init");
  git(dir, "remote", "add", "origin", remote);
  git(dir, "push", "-q", "-u", "origin", branch);
  return dir;
}

async function publishJev(cfg: string, host: "claude" | "codex", extra: EnvPatch = {}) {
  const env =
    host === "claude"
      ? { CLAUDE_CONFIG_DIR: cfg, TOOLU_HOST_OVERRIDE: "claude", ...extra }
      : { CODEX_HOME: cfg, TOOLU_HOST_OVERRIDE: "codex", ...extra };
  const res = await run([process.execPath, JEV_HOOK], { env, stdin: "" });
  expect(res.exitCode).toBe(0);
}

const PAYLOAD = (cwd: string, extra = ""): string =>
  `{"model":{"display_name":"Opus"},"workspace":{"current_dir":${JSON.stringify(cwd)}},"context_window":{"context_window_size":200000,"total_input_tokens":1000}${extra}}`;

const RENDER_CASES: Record<string, Setup> = {
  "model and ctx": () => ({
    payload:
      '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":45000,"used_percentage":22}}',
  }),
  "effort present": () => ({
    payload:
      '{"model":{"display_name":"Opus"},"effort":{"level":"high"},"context_window":{"context_window_size":200000,"total_input_tokens":1000}}',
  }),
  "gate failing, no repo": (sb) => {
    sb.write(".claude/tmp/quality-gate-status.json", '{"status":"failing","reason":"x"}');
    return {};
  },
  "gate passing": (sb) => {
    sb.write(".claude/tmp/quality-gate-status.json", '{"status":"passing"}');
    return {};
  },
  "no gate file": () => ({}),
  "gate file garbage": (sb) => {
    sb.write(".claude/tmp/quality-gate-status.json", "garbage");
    return {};
  },
  "clean repo": (sb) => (repo(sb.project), {}),
  "staged": (sb) => {
    repo(sb.project);
    sb.write("s.txt", "");
    git(sb.project, "add", "s.txt");
    return {};
  },
  "mixed dirty": (sb) => {
    repo(sb.project);
    sb.write("f.txt", "c\n");
    git(sb.project, "add", "f.txt");
    git(sb.project, "commit", "-qm", "add");
    sb.write("f.txt", "c\nch\n");
    sb.write("s.txt", "");
    git(sb.project, "add", "s.txt");
    sb.write("u.txt", "");
    sb.write("dir/deep/u2.txt", "");
    return {};
  },
  "ahead + untracked": (sb) => {
    const dir = withRemote(sb);
    git(dir, "commit", "-q", "--allow-empty", "-m", "ahead");
    put(join(dir, "new.txt"), "");
    return { cwd: dir };
  },
  "behind": (sb) => {
    const dir = withRemote(sb);
    git(dir, "commit", "-q", "--allow-empty", "-m", "behind");
    git(dir, "push", "-q", "origin", "main");
    git(dir, "reset", "-q", "--hard", "HEAD~1");
    return { cwd: dir };
  },
  "ahead and behind": (sb) => {
    const dir = withRemote(sb);
    git(dir, "commit", "-q", "--allow-empty", "-m", "r");
    git(dir, "push", "-q", "origin", "main");
    git(dir, "reset", "-q", "--hard", "HEAD~1");
    git(dir, "commit", "-q", "--allow-empty", "-m", "l");
    return { cwd: dir };
  },
  "gone upstream": (sb) => {
    const dir = withRemote(sb);
    git(dir, "checkout", "-q", "-b", "feat");
    git(dir, "push", "-q", "-u", "origin", "feat");
    git(dir, "push", "-q", "origin", "--delete", "feat");
    return { cwd: dir };
  },
  "detached HEAD dirty": (sb) => {
    repo(sb.project);
    git(sb.project, "checkout", "-q", "--detach");
    sb.write("u.txt", "");
    return {};
  },
  "unborn repo": (sb) => {
    git(sb.project, "init", "-q", "-b", "trunk");
    sb.write("u.txt", "");
    return {};
  },
  "subdir gate": (sb) => {
    repo(sb.project);
    sb.write(".claude/tmp/quality-gate-status.json", '{"status":"failing","reason":"x"}');
    mkdirSync(sb.path("packages/app/src"), { recursive: true });
    return { cwd: sb.path("packages/app/src") };
  },
  "codex host reads .codex gate": (sb) => {
    repo(sb.project);
    sb.write(".codex/tmp/quality-gate-status.json", '{"status":"failing","reason":"c"}');
    sb.write(".claude/tmp/quality-gate-status.json", '{"status":"passing"}');
    return { env: { PLUGIN_ROOT: PLUGIN, CODEX_HOME: sb.codexHome } };
  },
  "cursor override behaves as claude": (sb) => {
    sb.write(".claude/tmp/quality-gate-status.json", '{"status":"failing"}');
    return { env: { TOOLU_HOST_OVERRIDE: "cursor" } };
  },
  "M-tier tokens": () => ({
    payload:
      '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":13779513,"used_percentage":99}}',
  }),
  "k-tier tokens": () => ({
    payload:
      '{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":13779,"used_percentage":7}}',
  }),
  "account domain": (_sb, cfg) => {
    put(join(cfg, ".claude.json"), '{"oauthAccount":{"emailAddress":"hello@example.com"}}');
    return {};
  },
  "account without oauth": (_sb, cfg) => (put(join(cfg, ".claude.json"), "{}"), {}),
  "account never falls back to HOME": (sb) => {
    put(join(sb.home, ".claude.json"), '{"oauthAccount":{"emailAddress":"a@leak.example"}}');
    return {};
  },
  "account from HOME without config dir": (sb) => {
    put(join(sb.home, ".claude.json"), '{"oauthAccount":{"emailAddress":"a@b@home.example"}}');
    return { env: { CLAUDE_CONFIG_DIR: undefined } };
  },
  "comemory marker": (sb, cfg) => {
    repo(sb.project);
    put(join(cfg, "comemory-status", "project.json"), '{"repo":"project","count":7}');
    return {};
  },
  "comemory zero": (sb, cfg) => {
    repo(sb.project);
    put(join(cfg, "comemory-status", "project.json"), '{"count":0}');
    return {};
  },
  "comemory worktree": (sb, cfg) => {
    const main = repo(join(sb.root, "main"));
    git(main, "worktree", "add", "-q", join(sb.root, "wt"));
    put(join(cfg, "comemory-status", "main.json"), '{"count":5}');
    return { cwd: join(sb.root, "wt") };
  },
  "comemory subdir": (sb, cfg) => {
    repo(sb.project);
    mkdirSync(sb.path("a/b"), { recursive: true });
    put(join(cfg, "comemory-status", "project.json"), '{"count":3}');
    return { cwd: sb.path("a/b") };
  },
  "comemory no marker": (sb) => (repo(sb.project), {}),
  "jev ready": async (_sb, cfg) => (await publishJev(cfg, "claude"), {}),
  "jev missing key": async (_sb, cfg) => {
    await publishJev(cfg, "claude");
    return { env: { TYPESAFE_API_KEY: undefined } };
  },
  "jev empty key": async (_sb, cfg) => {
    await publishJev(cfg, "claude");
    return { env: { TYPESAFE_API_KEY: "" } };
  },
  "jev newline key": async (_sb, cfg) => {
    await publishJev(cfg, "claude");
    return { env: { TYPESAFE_API_KEY: "secret\nvalue" } };
  },
  "jev CR key": async (_sb, cfg) => {
    await publishJev(cfg, "claude");
    return { env: { TYPESAFE_API_KEY: "\r" } };
  },
  "jev broken symlink": (sb, cfg) => {
    mkdirSync(join(cfg, "jev"), { recursive: true });
    symlinkSync(join(sb.root, "removed/jev.sh"), join(cfg, "jev/jev.sh"));
    return {};
  },
  "jev non-executable": (_sb, cfg) => {
    mkdirSync(join(cfg, "jev"), { recursive: true });
    copyFileSync(JEV_SH, join(cfg, "jev/jev.sh"));
    chmodSync(join(cfg, "jev/jev.sh"), 0o644);
    return {};
  },
  "jev directory": (_sb, cfg) => (mkdirSync(join(cfg, "jev/jev.sh"), { recursive: true }), {}),
  "jev missing curl and key": async (sb, cfg) => {
    await publishJev(cfg, "claude");
    const bin = join(sb.root, "bin");
    mkdirSync(bin);
    for (const tool of ["bash", "jq", "git", "basename", "dirname", "cat", "readlink", "bun"]) {
      const found = Bun.which(tool);
      if (found !== null) symlinkSync(found, join(bin, tool));
    }
    return { env: { PATH: bin, TYPESAFE_API_KEY: undefined } };
  },
  "jev codex wrapper only": async (sb) => (await publishJev(sb.codexHome, "codex"), {}),
  "jev explicit config root": async (sb) => {
    const explicit = join(sb.root, "explicit profile");
    await publishJev(explicit, "claude", { TOOLU_CONFIG_DIR: explicit });
    return { env: { TOOLU_CONFIG_DIR: explicit } };
  },
  "no workspace payload": async (_sb, cfg) => {
    await publishJev(cfg, "claude");
    return { payload: "{}" };
  },
  "apostrophe path": async (sb, cfg) => {
    await publishJev(cfg, "claude");
    const dir = join(sb.root, "user's project");
    mkdirSync(dir);
    return { cwd: dir };
  },
  "top-level cwd field": (sb) => ({ payload: `{"cwd":${JSON.stringify(sb.project)}}` }),
  "string model aborts jq": () => ({
    payload:
      '{"model":"Opus","context_window":{"context_window_size":200000,"total_input_tokens":45000}}',
  }),
  "string effort keeps model": (sb) => ({
    payload: `{"model":{"display_name":"M"},"effort":"x","workspace":{"current_dir":${JSON.stringify(sb.project)}}}`,
  }),
  "array payload": () => ({ payload: "[1,2]" }),
  "string payload": () => ({ payload: '"str"' }),
  "invalid json": () => ({ payload: "not json" }),
  "empty stdin": () => ({ payload: "" }),
  "null payload": () => ({ payload: "null" }),
  "mixed types, 22.5%": () => ({
    payload:
      '{"model":{"display_name":""},"effort":{"level":false},"context_window":{"context_window_size":"200000","total_input_tokens":1500000.5,"used_percentage":22.5}}',
  }),
  "null effort text, 23.5%": () => ({
    payload: '{"effort":{"level":"null"},"context_window":{"used_percentage":23.5}}',
  }),
  "numeric effort, bool tokens, string pct": () => ({
    payload:
      '{"effort":{"level":3},"context_window":{"total_input_tokens":true,"used_percentage":"7"}}',
  }),
  "half percents": () => ({
    payload: '{"context_window":{"used_percentage":0.5,"context_window_size":999}}',
  }),
  "workspace is string": () => ({ payload: '{"model":{"display_name":"M"},"workspace":"x"}' }),
  "nonexistent cwd": (sb) => ({ cwd: join(sb.root, "missing") }),
  "everything": async (sb, cfg) => {
    const dir = withRemote(sb);
    git(dir, "commit", "-q", "--allow-empty", "-m", "ahead");
    put(join(dir, "u.txt"), "");
    put(join(dir, ".claude/tmp/quality-gate-status.json"), '{"status":"failing"}');
    put(join(cfg, ".claude.json"), '{"oauthAccount":{"emailAddress":"me@corp.example"}}');
    put(join(cfg, "comemory-status", "repo.json"), '{"count":42}');
    await publishJev(cfg, "claude");
    return {
      cwd: dir,
      payload: `{"model":{"display_name":"Opus 4"},"effort":{"level":"high"},"workspace":{"current_dir":${JSON.stringify(dir)}},"context_window":{"context_window_size":1000000,"total_input_tokens":456789,"used_percentage":45.6}}`,
    };
  },
};

function baseEnv(sb: Sandbox, cfg: string): EnvPatch {
  return { HOME: sb.home, CLAUDE_CONFIG_DIR: cfg, TYPESAFE_API_KEY: "statusline-test-key" };
}

for (const [name, setup] of Object.entries(RENDER_CASES)) {
  test.concurrent(`renderer parity: ${name}`, async () => {
    using sb = createSandbox();
    const cfg = join(sb.root, "cfg");
    const got = (await setup(sb, cfg)) ?? {};
    const cwd = got.cwd ?? sb.project;
    const payload = got.payload ?? PAYLOAD(cwd);
    const env = { ...baseEnv(sb, cfg), ...got.env };
    const opts = { cwd: sb.project, env, stdin: payload };
    const [bash, bun] = await Promise.all([
      run(["/bin/bash", join(PLUGIN, "statusline.sh")], opts),
      run([RENDER], opts),
    ]);
    expect(bun.exitCode).toBe(0);
    expect(bun.stdout).toBe(bash.stdout);
  });
}

const REPORT_CASES: Record<string, Setup> = {
  "clean repo": (sb) => (repo(sb.project), {}),
  "dirty repo": (sb) => {
    repo(sb.project);
    sb.write("u.txt", "");
    sb.write("s.txt", "");
    git(sb.project, "add", "s.txt");
    return {};
  },
  "ahead and behind": (sb) => {
    const dir = withRemote(sb);
    git(dir, "commit", "-q", "--allow-empty", "-m", "r");
    git(dir, "push", "-q", "origin", "main");
    git(dir, "reset", "-q", "--hard", "HEAD~1");
    git(dir, "commit", "-q", "--allow-empty", "-m", "l");
    return { cwd: dir };
  },
  "detached": (sb) => {
    repo(sb.project);
    git(sb.project, "checkout", "-q", "--detach");
    return {};
  },
  "not a repo": () => ({}),
  "gate failing with reason": (sb) => {
    repo(sb.project);
    sb.write(".codex/tmp/quality-gate-status.json", '{"status":"failing","reason":"codex state"}');
    sb.write(".claude/tmp/quality-gate-status.json", '{"status":"passing"}');
    return {};
  },
  "gate failing no reason": (sb) => {
    sb.write(".codex/tmp/quality-gate-status.json", '{"status":"failing"}');
    return {};
  },
  "gate passing": (sb) => {
    sb.write(".codex/tmp/quality-gate-status.json", '{"status":"passing"}');
    return {};
  },
  "gate unknown status": (sb) => {
    sb.write(".codex/tmp/quality-gate-status.json", '{"status":"stale"}');
    return {};
  },
  "comemory": (sb) => {
    repo(sb.project);
    put(join(sb.codexHome, "comemory-status", "project.json"), '{"count":7}');
    return {};
  },
  "jev ready": async (sb) => (await publishJev(sb.codexHome, "codex"), {}),
  "jev unavailable": async (sb) => {
    await publishJev(sb.codexHome, "codex");
    return { env: { TYPESAFE_API_KEY: "" } };
  },
  "jev claude wrapper ignored": async (_sb, cfg) => (await publishJev(cfg, "claude"), {}),
  "jev explicit root": async (sb) => {
    const explicit = join(sb.root, "explicit profile");
    await publishJev(explicit, "claude", { TOOLU_CONFIG_DIR: explicit });
    return { env: { TOOLU_CONFIG_DIR: explicit } };
  },
};

for (const [name, setup] of Object.entries(REPORT_CASES)) {
  test.concurrent(`report parity: ${name}`, async () => {
    using sb = createSandbox();
    const cfg = join(sb.root, "cfg");
    const got = (await setup(sb, cfg)) ?? {};
    const cwd = got.cwd ?? sb.project;
    const env = { ...baseEnv(sb, cfg), CODEX_HOME: sb.codexHome, ...got.env };
    const [bash, bun, bashDefault, bunDefault] = await Promise.all([
      run(["/bin/bash", join(PLUGIN, "scripts/status.sh"), cwd], { cwd: sb.root, env }),
      run([process.execPath, join(PLUGIN, "hooks/dist/status.js"), cwd], { cwd: sb.root, env }),
      run(["/bin/bash", join(PLUGIN, "scripts/status.sh")], { cwd, env }),
      run([process.execPath, join(PLUGIN, "hooks/dist/status.js")], { cwd, env }),
    ]);
    expect(bash.exitCode).toBe(0);
    expect({ exit: bun.exitCode, out: bun.stdout }).toEqual({ exit: 0, out: bash.stdout });
    expect(bunDefault.stdout).toBe(bashDefault.stdout);
  });
}

type SetupCase = { before?: string; args?: string[]; defaultDir?: boolean; twice?: boolean };

const SETUP_CASES: Record<string, SetupCase> = {
  "absent settings": {},
  "existing keys": { before: '{\n  "theme": "dark"\n}\n' },
  "idempotent": { twice: true },
  "custom refused": {
    before: '{\n  "statusLine": { "type": "command", "command": "my-custom-bar" }\n}\n',
  },
  "custom forced": {
    before: '{\n  "statusLine": { "type": "command", "command": "my-custom-bar" }\n}\n',
    args: ["--force"],
  },
  "custom forced -f": {
    before: '{\n  "statusLine": { "type": "command", "command": "my-custom-bar" }\n}\n',
    args: ["-f"],
  },
  "unparseable": { before: "not json {{{" },
  "non-object": { before: "[1]" },
  "empty file": { before: "" },
  "null statusLine": { before: '{"statusLine": null, "a": 1}' },
  "default dir": { defaultDir: true },
};

/** Run one setup implementation in its own config dir; report what a user observes. */
async function observeSetup(argv: string[], sb: Sandbox, tag: string, c: SetupCase) {
  const home = join(sb.root, `home-${tag}`);
  const cfg = c.defaultDir === true ? join(home, ".claude") : join(sb.root, `cfg-${tag}`);
  mkdirSync(cfg, { recursive: true });
  const settings = join(cfg, "settings.json");
  if (c.before !== undefined) writeFileSync(settings, c.before);
  const env = { HOME: home, CLAUDE_CONFIG_DIR: c.defaultDir === true ? undefined : cfg };
  if (c.twice === true) await run([...argv, ...(c.args ?? [])], { env });
  const res = await run([...argv, ...(c.args ?? [])], { env });
  const parsed = existsSync(settings) ? readFileSync(settings, "utf8") : "";
  const token = res.stdout.split(" ")[0];
  const firstLine = res.stdout.split("\n")[0]?.replaceAll(cfg, "<cfg>");
  let doc: unknown = parsed;
  try {
    doc = JSON.parse(parsed);
  } catch {
    doc = parsed;
  }
  return { exit: res.exitCode, token, firstLine, doc, bak: existsSync(`${settings}.bak`), cfg };
}

for (const [name, c] of Object.entries(SETUP_CASES)) {
  test.concurrent(`setup parity: ${name}`, async () => {
    using sb = createSandbox();
    const bash = await observeSetup(["/bin/bash", join(PLUGIN, "scripts/setup.sh")], sb, "bash", c);
    const bun = await observeSetup(
      [process.execPath, join(PLUGIN, "hooks/dist/setup.js")],
      sb,
      "bun",
      c,
    );
    const expected = JSON.parse(
      JSON.stringify(bash.doc).replaceAll(bash.cfg, bun.cfg).replace(/"bash /g, '"'),
    );
    expect({ exit: bun.exit, token: bun.token, bak: bun.bak, doc: bun.doc }).toEqual({
      exit: bash.exit,
      token: bash.token,
      bak: bash.bak,
      doc: expected,
    });
    if (bash.token !== "ERROR") expect(bun.firstLine).toBe(bash.firstLine);
  });
}
