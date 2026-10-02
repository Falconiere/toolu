/**
 * Every way a startup entry can fail leaves the bootstrap NotReady with the
 * plugin, entry and cause (#276, #342). Fixture plugins carry the generated
 * launcher `hooks.json` and a small real bundle run by the real Bun.
 */
import { expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { STARTUP_REPORT_ENV } from "@toolu/core/startup";
import type { PluginManifest } from "../../inventory/types.ts";
import { STARTUP_REPORT_VAR, bootstrapRuntime, type BootstrapRuntimeOptions } from "../runtime.ts";
import { REPO_ROOT, fixturePlugin, tempRoot, type FixtureOptions } from "./fixtures.ts";

type Boot = Partial<BootstrapRuntimeOptions> & { plugins: PluginManifest[] };

function boot(root: string, options: Boot): ReturnType<typeof bootstrapRuntime> {
  return bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: root,
    dataRoot: join(root, "data"),
    isolatedHome: join(root, "home"),
    ...options,
  });
}

function one(root: string, source: string, options: FixtureOptions = {}): PluginManifest {
  return fixturePlugin(root, "probe", { entries: { boot: source }, ...options });
}

async function reasonOf(root: string, options: Boot): Promise<string> {
  const result = await boot(root, options);
  if (result.status === "ready") throw new Error("expected not-ready");
  return result.reason;
}

const REPORT = 'const fs = require("node:fs"); const report = process.env.TOOLU_STARTUP_REPORT;\n';

test.concurrent("the bootstrap names the report file in the variable core's writers read", () => {
  expect(STARTUP_REPORT_VAR).toBe(STARTUP_REPORT_ENV);
});

test.concurrent("#326: bootstrap runs the TOOLU_BUN executable with a restricted PATH", async () => {
  using root = tempRoot("toolu-bs-bun-");
  const marker = join(root.path, "selected-bun");
  const wrapper = join(root.path, "bun wrapper");
  writeFileSync(
    wrapper,
    '#!/bin/sh\nprintf "selected\\n" > "$TOOLU_BUN_MARKER"\nexec "$TOOLU_REAL_BUN" "$@"\n',
  );
  chmodSync(wrapper, 0o755);
  const env = {
    PATH: root.path,
    HOME: root.path,
    TOOLU_BUN: wrapper,
    TOOLU_BUN_MARKER: marker,
    TOOLU_REAL_BUN: process.execPath,
  };
  const result = await boot(root.path, { plugins: [one(root.path, "process.exit(0);\n")], env });
  expect(result).toMatchObject({
    status: "ready",
    plugins: [{ plugin: "probe", entries: [{ entry: "boot" }] }],
  });
  expect(readFileSync(marker, "utf8")).toBe("selected\n");
});

test.concurrent("#326: bootstrap finds Bun in the host HOME before isolating child HOME", async () => {
  using root = tempRoot("toolu-bs-bun-home-");
  const bunDir = join(root.path, "host-home", ".bun", "bin");
  mkdirSync(bunDir, { recursive: true });
  symlinkSync(process.execPath, join(bunDir, "bun"));
  const env = { PATH: root.path, HOME: join(root.path, "host-home"), TOOLU_BUN: "" };
  const result = await boot(root.path, { plugins: [one(root.path, "process.exit(0);\n")], env });
  expect(result.status).toBe("ready");
});

test.concurrent("an entry that cannot start, exits non-zero or stalls is NotReady", async () => {
  using root = tempRoot("toolu-bs-exit-");
  const failing = one(
    root.path,
    'process.stderr.write("registration failed\\n"); process.exit(7);\n',
  );
  expect(await reasonOf(root.path, { plugins: [failing] })).toBe(
    "probe/boot: exited 7: registration failed",
  );
  const missingCwd = join(root.path, "missing-working-directory");
  expect(await reasonOf(root.path, { plugins: [failing], projectRoot: missingCwd })).toStartWith(
    "probe/boot: cannot start: ",
  );
  using slow = tempRoot("toolu-bs-timeout-");
  const stalled = one(slow.path, "await Bun.sleep(5000);\n");
  expect(await reasonOf(slow.path, { plugins: [stalled], deadlineMs: 300 })).toBe(
    "probe/boot: timed out after 300 ms",
  );
});

test.concurrent("a grandchild holding the output pipes cannot outlast the deadline", async () => {
  using root = tempRoot("toolu-bs-grandchild-");
  const holder =
    'require("node:child_process").spawn("sleep", ["8"], { stdio: "inherit", detached: true }).unref();\n' +
    "await Bun.sleep(5000);\n";
  const started = performance.now();
  const reason = await reasonOf(root.path, { plugins: [one(root.path, holder)], deadlineMs: 300 });
  expect(reason).toBe("probe/boot: timed out after 300 ms");
  expect(performance.now() - started).toBeLessThan(4_000);
});

test.concurrent("an abort kills the running entry, and an aborted signal runs nothing", async () => {
  using root = tempRoot("toolu-bs-abort-");
  const stalled = one(root.path, "await Bun.sleep(5000);\n");
  const started = performance.now();
  const reason = await reasonOf(root.path, {
    plugins: [stalled],
    signal: AbortSignal.timeout(200),
  });
  expect(reason).toBe("probe/boot: startup cancelled");
  expect(performance.now() - started).toBeLessThan(4_000);
  using idle = tempRoot("toolu-bs-aborted-");
  const marker = join(idle.path, "ran");
  const writes = one(
    idle.path,
    `require("node:fs").writeFileSync(${JSON.stringify(marker)}, "");\n`,
  );
  const aborted = AbortSignal.abort();
  expect(await reasonOf(idle.path, { plugins: [writes], signal: aborted })).toBe(
    "probe/boot: startup cancelled",
  );
  expect(existsSync(marker)).toBe(false);
});

test.concurrent("flooded, non-JSON or foreign-shaped stdout is NotReady", async () => {
  using root = tempRoot("toolu-bs-output-");
  const flood = one(root.path, 'process.stdout.write("x".repeat(600000));\n');
  expect(await reasonOf(root.path, { plugins: [flood] })).toBe(
    "probe/boot: startup output exceeded 512000 bytes",
  );
  using text = tempRoot("toolu-bs-text-");
  const plain = one(text.path, 'process.stdout.write("hello\\n");\n');
  expect(await reasonOf(text.path, { plugins: [plain] })).toBe(
    "probe/boot: invalid startup output: not JSON",
  );
  using shaped = tempRoot("toolu-bs-shape-");
  const extra = one(shaped.path, "process.stdout.write(JSON.stringify({ continue: false }));\n");
  expect(await reasonOf(shaped.path, { plugins: [extra] })).toStartWith(
    "probe/boot: invalid startup output: ",
  );
});

test.concurrent("SessionStart context is collected and bounded", async () => {
  using root = tempRoot("toolu-bs-context-");
  const text = "y".repeat(20_000);
  const output = {
    hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: text },
    systemMessage: "hi",
  };
  const talker = one(
    root.path,
    `process.stdout.write(${JSON.stringify(JSON.stringify(output))});\n`,
  );
  const result = await boot(root.path, { plugins: [talker] });
  if (result.status !== "ready") throw new Error(result.reason);
  const entry = result.plugins[0]?.entries[0];
  expect(entry?.systemMessage).toBe("hi");
  expect(entry?.additionalContext).toBe("y".repeat(10_000));
});

test.concurrent("a successful entry's stderr becomes a startup note", async () => {
  using root = tempRoot("toolu-bs-stderr-");
  const noisy = one(root.path, 'process.stderr.write("bun not found on PATH\\n");\n');
  const result = await boot(root.path, { plugins: [noisy] });
  if (result.status !== "ready") throw new Error(result.reason);
  expect(result.diagnostics).toEqual(["probe/boot: bun not found on PATH"]);
});

test.concurrent("an invalid, failed or foreign report record is NotReady", async () => {
  using root = tempRoot("toolu-bs-report-");
  const garbage = one(root.path, `${REPORT}fs.appendFileSync(report, "garbage\\n");\n`);
  expect(await reasonOf(root.path, { plugins: [garbage] })).toStartWith(
    "probe/boot: invalid startup report: ",
  );
  using helper = tempRoot("toolu-bs-helper-");
  const missing = {
    kind: "helper",
    plugin: "probe",
    source: "/nowhere.js",
    status: "source-missing",
  };
  const unpublished = one(
    helper.path,
    `${REPORT}fs.appendFileSync(report, ${JSON.stringify(`${JSON.stringify(missing)}\n`)});\n`,
  );
  expect(await reasonOf(helper.path, { plugins: [unpublished] })).toBe(
    "probe/boot: helper /nowhere.js: source-missing",
  );
  using foreign = tempRoot("toolu-bs-foreign-");
  const target = join(foreign.path, "data", "toolu", "post-tools.d", "other@toolu__m.js");
  const record = {
    kind: "registry",
    spec: "other@toolu",
    name: "m",
    event: "tool/post",
    source: target,
    target,
    status: "written",
  };
  const lying = one(
    foreign.path,
    `${REPORT}fs.appendFileSync(report, ${JSON.stringify(`${JSON.stringify(record)}\n`)});\n`,
  );
  expect(await reasonOf(foreign.path, { plugins: [lying] })).toBe(
    `probe/boot: ${target}: contribution outside probe`,
  );
});

/** A bundle that writes `file` (relative to the data root) and reports `record` about it under `key`. */
function claims(record: object, key: "target" | "path", file: string, body: string): string {
  return (
    `${REPORT}const path = require("node:path");\n` +
    `const target = path.join(process.env.TOOLU_CONFIG_DIR, ${JSON.stringify(file)});\n` +
    "fs.mkdirSync(path.dirname(target), { recursive: true });\n" +
    `fs.writeFileSync(target, ${JSON.stringify(body)});\n` +
    `fs.appendFileSync(report, JSON.stringify({ ...${JSON.stringify(record)}, ${key}: target, source: path.join(process.env.CLAUDE_PLUGIN_ROOT, "hooks/dist/boot.js") }) + "\\n");\n`
  );
}

test.concurrent("a reported artifact that is not the current bundle or link is NotReady", async () => {
  using root = tempRoot("toolu-bs-stale-");
  const module = "toolu/post-tools.d/probe@toolu__m.js";
  const registry = {
    kind: "registry",
    spec: "probe@toolu",
    name: "m",
    event: "tool/post",
    status: "unchanged",
  };
  const stale = one(root.path, claims(registry, "target", module, "// not the bundle\n"));
  const target = join(root.path, "data", module);
  expect(await reasonOf(root.path, { plugins: [stale] })).toBe(
    `probe/boot: ${target} is not the current bundle`,
  );
  using link = tempRoot("toolu-bs-link-");
  const helper = { kind: "helper", plugin: "probe", status: "published" };
  const notLink = one(link.path, claims(helper, "path", "probe/probe.sh", "#!/bin/sh\n"));
  const path = join(link.path, "data", "probe/probe.sh");
  const source = join(notLink.pluginDir, "hooks/dist/boot.js");
  expect(await reasonOf(link.path, { plugins: [notLink] })).toBe(
    `probe/boot: helper ${path} is not a link to ${source}`,
  );
});

test.concurrent("a failed dependency skips its dependents", async () => {
  using root = tempRoot("toolu-bs-deps-");
  const marker = join(root.path, "leaf-ran");
  const base = fixturePlugin(root.path, "base", { entries: { boot: "process.exit(3);\n" } });
  const leaf = fixturePlugin(root.path, "leaf", {
    entries: { boot: `require("node:fs").writeFileSync(${JSON.stringify(marker)}, "");\n` },
    dependencies: ["base"],
  });
  expect(await reasonOf(root.path, { plugins: [leaf, base] })).toBe(
    "base/boot: exited 3; leaf: skipped, dependency base failed",
  );
  expect(existsSync(marker)).toBe(false);
});

test.concurrent("#276: a selected legacy-only register hook is NotReady", async () => {
  using root = tempRoot("toolu-bs-legacy-");
  const hooksJson = JSON.stringify({
    hooks: {
      SessionStart: [
        { hooks: [{ type: "command", command: 'bash "${CLAUDE_PLUGIN_ROOT}/hooks/register.sh"' }] },
      ],
    },
  });
  const legacy = fixturePlugin(root.path, "legacy-only", { hooksJson });
  expect(await reasonOf(root.path, { plugins: [legacy] })).toBe(
    'legacy-only: unsupported SessionStart command "bash \\"${CLAUDE_PLUGIN_ROOT}/hooks/register.sh\\""; ' +
      "regenerate it with `bun run tooling/src/check-hooks-json.ts --print legacy-only SessionStart <entry>`",
  );
});

test.concurrent("#276: an unwritable startup root becomes NotReady", async () => {
  using root = tempRoot("toolu-bs-root-");
  const dataRoot = join(root.path, "occupied-root");
  writeFileSync(dataRoot, "occupied\n");
  const reason = await reasonOf(root.path, { plugins: [], dataRoot });
  expect(reason).toStartWith("bootstrap failed: ");
  expect(readFileSync(dataRoot, "utf8")).toBe("occupied\n");
});
