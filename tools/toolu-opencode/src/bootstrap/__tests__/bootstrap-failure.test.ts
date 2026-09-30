import { expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { PluginManifest } from "../../inventory/types.ts";
import { bootstrapRuntime } from "../runtime.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

function plugin(name: string, pluginDir: string): PluginManifest {
  return {
    name,
    spec: `${name}@toolu`,
    marketplace: "toolu",
    version: "1",
    pluginDir,
    dependencies: [],
  };
}

function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
}

test("#276: a bundle that cannot start leaves bootstrap NotReady", async () => {
  const project = mkdtempSync(join(tmpBase, "toolu-bs-spawn-"));
  const pluginDir = mkdtempSync(join(tmpBase, "toolu-bs-spawn-plugin-"));
  mkdirSync(join(pluginDir, "hooks", "dist"), { recursive: true });
  writeFileSync(join(pluginDir, "hooks", "dist", "register.js"), "process.exit(0);\n");
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot: join(project, "missing-working-directory"),
    dataRoot: mkdtempSync(join(tmpBase, "toolu-bs-spawn-data-")),
    plugins: [plugin("spawn-failure", pluginDir)],
  });
  expect(result.status).toBe("not-ready");
  if (result.status === "not-ready") expect(result.reason).toContain("bootstrap bundle failed");
});

test("#276: a missing register bundle blocks a later session-start bundle", async () => {
  const pluginDir = mkdtempSync(join(tmpBase, "toolu-bs-register-gap-"));
  mkdirSync(join(pluginDir, "hooks", "dist"), { recursive: true });
  writeFileSync(join(pluginDir, "hooks", ".requires-native-register"), "\n");
  writeFileSync(join(pluginDir, "hooks", "dist", "session-start.js"), "process.exit(0);\n");
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot: mkdtempSync(join(tmpBase, "toolu-bs-register-gap-project-")),
    dataRoot: mkdtempSync(join(tmpBase, "toolu-bs-register-gap-data-")),
    plugins: [plugin("register-gap", pluginDir)],
  });
  expect(result.status).toBe("not-ready");
  if (result.status === "not-ready") expect(result.reason).toContain("no native startup bundle");
});

test("#276: a selected legacy-only register hook is NotReady", async () => {
  const pluginDir = mkdtempSync(join(tmpBase, "toolu-bs-legacy-plugin-"));
  mkdirSync(join(pluginDir, "hooks"), { recursive: true });
  writeFileSync(join(pluginDir, "hooks", "register.sh"), "#!/usr/bin/env bash\nexit 0\n");
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot: mkdtempSync(join(tmpBase, "toolu-bs-legacy-project-")),
    dataRoot: mkdtempSync(join(tmpBase, "toolu-bs-legacy-data-")),
    plugins: [plugin("legacy-only", pluginDir)],
  });
  expect(result.status).toBe("not-ready");
  if (result.status === "not-ready") expect(result.reason).toContain("no native startup bundle");
});

test("#276: excessive bundle output is bounded and leaves bootstrap NotReady", async () => {
  const pluginDir = mkdtempSync(join(tmpBase, "toolu-bs-output-plugin-"));
  mkdirSync(join(pluginDir, "hooks", "dist"), { recursive: true });
  writeFileSync(
    join(pluginDir, "hooks", "dist", "register.js"),
    'process.stdout.write("x".repeat(600000));\n',
  );
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot: mkdtempSync(join(tmpBase, "toolu-bs-output-project-")),
    dataRoot: mkdtempSync(join(tmpBase, "toolu-bs-output-data-")),
    plugins: [plugin("excess-output", pluginDir)],
  });
  expect(result.status).toBe("not-ready");
  if (result.status === "not-ready") expect(result.reason).toContain("output exceeded 512000");
});

test("#276: an unwritable startup root becomes NotReady", async () => {
  const projectRoot = mkdtempSync(join(tmpBase, "toolu-bs-root-project-"));
  const dataRoot = join(projectRoot, "occupied-root");
  writeFileSync(dataRoot, "occupied\n");
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot,
    dataRoot,
    plugins: [],
  });
  expect(result.status).toBe("not-ready");
  if (result.status === "not-ready") expect(result.reason).toContain("bootstrap failed");
  expect(readFileSync(dataRoot, "utf8")).toBe("occupied\n");
});

test("#276: a failing bundle preserves its exit status and error", async () => {
  const pluginDir = mkdtempSync(join(tmpBase, "toolu-bs-exit-plugin-"));
  mkdirSync(join(pluginDir, "hooks", "dist"), { recursive: true });
  writeFileSync(
    join(pluginDir, "hooks", "dist", "register.js"),
    'process.stderr.write("registration failed\\n"); process.exit(7);\n',
  );
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot: mkdtempSync(join(tmpBase, "toolu-bs-exit-project-")),
    dataRoot: mkdtempSync(join(tmpBase, "toolu-bs-exit-data-")),
    plugins: [plugin("exit-failure", pluginDir)],
  });
  expect(result.status).toBe("not-ready");
  if (result.status === "not-ready") {
    expect(result.reason).toContain("exited 7");
    expect(result.reason).toContain("registration failed");
  }
});

test("#276: a stalled bundle times out and leaves bootstrap NotReady", async () => {
  const pluginDir = mkdtempSync(join(tmpBase, "toolu-bs-timeout-plugin-"));
  mkdirSync(join(pluginDir, "hooks", "dist"), { recursive: true });
  writeFileSync(join(pluginDir, "hooks", "dist", "register.js"), "await Bun.sleep(5000);\n");
  const result = await bootstrapRuntime({
    repoRoot: repoRoot(),
    projectRoot: mkdtempSync(join(tmpBase, "toolu-bs-timeout-project-")),
    dataRoot: mkdtempSync(join(tmpBase, "toolu-bs-timeout-data-")),
    plugins: [plugin("timeout", pluginDir)],
    deadlineMs: 500,
  });
  expect(result.status).toBe("not-ready");
  if (result.status === "not-ready") expect(result.reason).toContain("timed out");
});
