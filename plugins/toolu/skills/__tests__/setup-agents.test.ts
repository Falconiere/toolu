/** setup.ts installing the real agent templates into sandboxed Codex homes. */

import { expect, test } from "bun:test";
import {
  appendFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { PROFILES } from "../setup/scripts/setup.ts";

const SCRIPT = join(import.meta.dir, "..", "setup", "scripts", "setup.ts");
const TEMPLATES = join(import.meta.dir, "..", "..", "assets", "agents");

const setup = (env: EnvPatch, ...args: string[]) => run(["bun", SCRIPT, ...args], { env });
const read = (path: string) => readFileSync(path, "utf8");

test.concurrent("agent templates contain the required models efforts sandboxes and instructions", () => {
  for (const { name, model, effort, sandbox } of PROFILES) {
    const lines = read(join(TEMPLATES, `${name}.toml`)).split("\n");
    expect(lines).toContain(`name = "${name}"`);
    expect(lines).toContain(`model = "${model}"`);
    expect(lines).toContain(`model_reasoning_effort = "${effort}"`);
    expect(lines).toContain(`sandbox_mode = "${sandbox}"`);
    expect(lines).toContain('developer_instructions = """');
  }
});

test.concurrent("preview falls back to HOME/.codex when CODEX_HOME is unset and writes nothing", async () => {
  using sb = createSandbox();
  const res = await setup({ CODEX_HOME: undefined, HOME: sb.home }, "preview");
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain(`TARGET ${sb.home}/.codex/agents\n`);
  expect(res.stdout).toContain("PLAN quick-task install\n");
  expect(existsSync(join(sb.home, ".codex", "agents"))).toBe(false);
});

test.concurrent("install writes all five profiles to an explicit CODEX_HOME", async () => {
  using sb = createSandbox();
  const codexHome = join(sb.root, "codex");
  const res = await setup({ CODEX_HOME: codexHome, HOME: sb.home }, "install");
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("INSTALLED 5 UPDATED 0 UNCHANGED 0\n");
  expect(res.stdout).toEndWith("Restart Codex to reload custom agent profiles.\n");
  for (const { name } of PROFILES) {
    expect(read(join(codexHome, "agents", `${name}.toml`))).toBe(
      read(join(TEMPLATES, `${name}.toml`)),
    );
    // The installed profile is the TOML Codex loads.
    expect(Bun.TOML.parse(read(join(codexHome, "agents", `${name}.toml`)))).toMatchObject({ name });
  }
});

test.concurrent("unchanged profiles are not backed up or rewritten", async () => {
  using sb = createSandbox();
  const env = { CODEX_HOME: join(sb.root, "codex"), HOME: sb.home };
  await setup(env, "install");
  const profile = join(env.CODEX_HOME, "agents", "quick-task.toml");
  const before = statSync(profile).mtimeMs;
  const res = await setup(env, "install");
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("UNCHANGED 5\n");
  expect(statSync(profile).mtimeMs).toBe(before);
  expect(existsSync(join(env.CODEX_HOME, "agents", ".toolu-backups"))).toBe(false);
});

test.concurrent("a changed managed profile is updated after a timestamped backup", async () => {
  using sb = createSandbox();
  const env = { CODEX_HOME: join(sb.root, "codex"), HOME: sb.home };
  const stamp = "20260813T190000Z";
  await setup(env, "install");
  const profile = join(env.CODEX_HOME, "agents", "quick-task.toml");
  writeFileSync(
    profile,
    read(profile).replace('model_reasoning_effort = "medium"', 'model_reasoning_effort = "low"'),
  );
  const res = await setup({ ...env, TOOLU_TIMESTAMP: stamp }, "install");
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("INSTALLED 0 UPDATED 1 UNCHANGED 4\n");
  const backup = join(env.CODEX_HOME, "agents", ".toolu-backups", stamp);
  expect(res.stdout).toContain(`BACKUP ${backup}\n`);
  expect(read(join(backup, "quick-task.toml")).split("\n")).toContain(
    'model_reasoning_effort = "low"',
  );
  expect(read(profile)).toBe(read(join(TEMPLATES, "quick-task.toml")));
});

test.concurrent("install refuses an unmanaged conflict without partially installing profiles", async () => {
  using sb = createSandbox();
  const codexHome = join(sb.root, "codex");
  mkdirSync(join(codexHome, "agents"), { recursive: true });
  writeFileSync(join(codexHome, "agents", "quick-task.toml"), 'name = "personal"\n');
  const res = await setup({ CODEX_HOME: codexHome, HOME: sb.home }, "install");
  expect(res.exitCode).toBe(2);
  expect(res.stdout).toContain("PLAN quick-task conflict\n");
  expect(res.stderr).toBe(
    "REFUSED unmanaged profile conflict; preview it and confirm install/remove --force\n",
  );
  expect(read(join(codexHome, "agents", "quick-task.toml"))).toBe('name = "personal"\n');
  expect(existsSync(join(codexHome, "agents", "architect.toml"))).toBe(false);
});

test.concurrent("confirmed force install backs up and replaces an unmanaged conflict", async () => {
  using sb = createSandbox();
  const codexHome = join(sb.root, "codex");
  const stamp = "20260813T190100Z";
  mkdirSync(join(codexHome, "agents"), { recursive: true });
  writeFileSync(join(codexHome, "agents", "quick-task.toml"), 'name = "personal"\n');
  const res = await setup(
    { CODEX_HOME: codexHome, HOME: sb.home, TOOLU_TIMESTAMP: stamp },
    "install",
    "--force",
  );
  expect(res.exitCode).toBe(0);
  expect(read(join(codexHome, "agents", ".toolu-backups", stamp, "quick-task.toml"))).toBe(
    'name = "personal"\n',
  );
  expect(read(join(codexHome, "agents", "quick-task.toml"))).toBe(
    read(join(TEMPLATES, "quick-task.toml")),
  );
});

test.concurrent("remove requires explicit confirmation and then preserves recoverable backups", async () => {
  using sb = createSandbox();
  const env = { CODEX_HOME: join(sb.root, "codex"), HOME: sb.home };
  const stamp = "20260813T190200Z";
  await setup(env, "install");
  const profile = join(env.CODEX_HOME, "agents", "quick-task.toml");

  const refused = await setup(env, "remove");
  expect(refused.exitCode).toBe(2);
  expect(refused.stderr).toBe("REFUSED removal requires --yes after explicit user confirmation\n");
  expect(existsSync(profile)).toBe(true);

  const res = await setup({ ...env, TOOLU_TIMESTAMP: stamp }, "remove", "--yes");
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("REMOVED 5 ABSENT 0\n");
  expect(existsSync(profile)).toBe(false);
  expect(
    existsSync(join(env.CODEX_HOME, "agents", ".toolu-backups", stamp, "quick-task.toml")),
  ).toBe(true);
});

test.concurrent("invalid template TOML fails before the destination is created", async () => {
  using sb = createSandbox();
  const templateDir = join(sb.root, "templates");
  cpSync(TEMPLATES, templateDir, { recursive: true });
  appendFileSync(join(templateDir, "architect.toml"), "invalid = [\n");
  const codexHome = join(sb.root, "codex");
  const res = await setup(
    { CODEX_HOME: codexHome, HOME: sb.home, TOOLU_AGENT_TEMPLATE_DIR: templateDir },
    "install",
  );
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toBe(`toolu setup: invalid agent template: ${templateDir}/architect.toml\n`);
  expect(existsSync(join(codexHome, "agents"))).toBe(false);
});

test.concurrent("an invalid backup timestamp fails before anything is written", async () => {
  using sb = createSandbox();
  const env = { CODEX_HOME: join(sb.root, "codex"), HOME: sb.home };
  await setup(env, "install");
  const res = await setup({ ...env, TOOLU_TIMESTAMP: "../x" }, "remove", "--yes");
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toBe("toolu setup: invalid backup timestamp: ../x\n");
  expect(existsSync(join(env.CODEX_HOME, "agents", "quick-task.toml"))).toBe(true);
});

test.concurrent("an unknown command or flag prints usage and exits 2", async () => {
  using sb = createSandbox();
  for (const args of [["deploy"], ["install", "--now"], []]) {
    const res = await setup({ CODEX_HOME: join(sb.root, "codex") }, ...args);
    expect(res.exitCode).toBe(2);
    expect(res.stderr).toBe(
      "Usage: setup.ts preview | install [--force] | remove --yes [--force]\n",
    );
  }
});

test.concurrent("no CODEX_HOME and no HOME fails with exit 1", async () => {
  const res = await setup({ CODEX_HOME: undefined, HOME: undefined }, "preview");
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toBe("toolu setup: CODEX_HOME and HOME are both unset\n");
});
