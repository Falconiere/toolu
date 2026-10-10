import { expect, test } from "bun:test";
import { chmodSync, cpSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { createSandbox } from "../sandbox.ts";
import { run } from "../spawn.ts";
import {
  bundlePath,
  entryArgv,
  implementationTag,
  installTooluShim,
  isCliEntry,
  launchedArgv,
  pluginName,
  pluginRoot,
  publishedArgv,
  requiredBuiltTooluBinary,
  resolveEntryCommand,
} from "../entry-command.ts";

const SAMPLE = bundlePath(pluginRoot("toolu"), "sample");
const ENTRY = { plugin: "toolu", entry: "pre-tools", bundle: SAMPLE };

test("native hook binary resolution fails clearly before spawn", () => {
  using sb = createSandbox();
  expect(() => requiredBuiltTooluBinary(sb.project)).toThrow(
    `toolu binary not found under ${join(sb.project, "target")}; build it first`,
  );
  const binary = sb.write("target/debug/toolu", "#!/bin/sh\nexit 0\n");
  expect(requiredBuiltTooluBinary(sb.project)).toBe(binary);
});

test("unset selector executes the committed Bun bundle unchanged", async () => {
  const command = resolveEntryCommand(ENTRY, {});
  expect(command).toEqual({ argv: [process.execPath, SAMPLE], implementation: "bun" });
  const result = await run(command.argv);
  expect(result.exitCode).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual({ kind: "allow" });
});

test("selected Rust command runs a real executable with argv and stdin intact", async () => {
  using sb = createSandbox();
  const binDir = sb.path("binary dir with spaces");
  const bin = sb.write(
    "binary dir with spaces/toolu",
    '#!/bin/sh\nprintf "%s\\n" "$@" > "$PROBE_ARGS"\ncat > "$PROBE_STDIN"\nprintf \'{"kind":"allow"}\\n\'\n',
  );
  chmodSync(bin, 0o755);
  const args = sb.path("args.txt");
  const stdin = sb.path("stdin.txt");
  const command = resolveEntryCommand(ENTRY, {
    TOOLU_IMPL: "rust:toolu/pre-tools",
    TOOLU_RUST_BIN_DIR: binDir,
  });
  expect(command).toEqual({ argv: [bin, "hook", "pre-tools"], implementation: "rust" });
  const result = await run(command.argv, {
    stdin: '{"fixture":"real"}',
    env: { PROBE_ARGS: args, PROBE_STDIN: stdin },
  });
  expect(result.exitCode).toBe(0);
  expect(readFileSync(args, "utf8")).toBe("hook\npre-tools\n");
  expect(readFileSync(stdin, "utf8")).toBe('{"fixture":"real"}');
  expect(JSON.parse(result.stdout)).toEqual({ kind: "allow" });
});

test("selected missing binary fails at resolution with selector and expected path", () => {
  using sb = createSandbox();
  const dir = sb.path("not-built");
  expect(() =>
    resolveEntryCommand(ENTRY, { TOOLU_IMPL: "rust:toolu/pre-tools", TOOLU_RUST_BIN_DIR: dir }),
  ).toThrow(`rust:toolu/pre-tools: binary not found at ${join(dir, "toolu")}`);
});

test("unselected entry keeps Bun even when the Rust directory is absent", async () => {
  using sb = createSandbox();
  const command = resolveEntryCommand(ENTRY, {
    TOOLU_IMPL: "rust:toolu/post-tools",
    TOOLU_RUST_BIN_DIR: sb.path("not-built"),
  });
  expect(command.implementation).toBe("bun");
  expect((await run(command.argv)).exitCode).toBe(0);
});

test("invalid selectors and non-executable binaries fail without fallback", () => {
  using sb = createSandbox();
  for (const selector of [
    "bun",
    "rust:",
    "rust:toolu/pre-tools,",
    "rust:toolu/pre-tools,toolu/pre-tools",
    "rust:toolu/../pre-tools",
  ]) {
    expect(() => resolveEntryCommand(ENTRY, { TOOLU_IMPL: selector })).toThrow("TOOLU_IMPL");
  }
  const bin = sb.write("bin/toolu", "#!/bin/sh\nexit 0\n");
  expect(() =>
    resolveEntryCommand(ENTRY, {
      TOOLU_IMPL: "rust",
      TOOLU_RUST_BIN_DIR: join(bin, ".."),
    }),
  ).toThrow("not executable");
});

test("a non-core plugin keeps its namespace and `rust` selects every entry", () => {
  using sb = createSandbox();
  const bin = sb.write("bin/toolu", "#!/bin/sh\nexit 0\n");
  chmodSync(bin, 0o755);
  const env = { TOOLU_IMPL: "rust", TOOLU_RUST_BIN_DIR: sb.path("bin") };
  const jev = {
    plugin: "jev",
    entry: "session-start",
    bundle: bundlePath(pluginRoot("jev"), "session-start"),
  };
  expect(resolveEntryCommand(jev, env)).toEqual({
    argv: [bin, "jev", "hook", "session-start"],
    implementation: "rust",
  });
  expect(resolveEntryCommand(ENTRY, env).argv).toEqual([bin, "hook", "pre-tools"]);
});

test("each argv helper keeps its default command and switches only when selected", () => {
  using sb = createSandbox();
  const bin = sb.write("bin/toolu", "#!/bin/sh\nexit 0\n");
  chmodSync(bin, 0o755);
  const rust = { TOOLU_IMPL: "rust:toolu/pre-tools", TOOLU_RUST_BIN_DIR: sb.path("bin") };
  const target = { plugin: "toolu", event: "PreToolUse", entry: "pre-tools" } as const;
  const bundle = bundlePath(pluginRoot("toolu"), "pre-tools");
  expect(launchedArgv(target, pluginRoot("toolu"), {})).toEqual([
    "/bin/sh",
    "-c",
    launcherCommand(target),
  ]);
  expect(entryArgv("toolu", "pre-tools", pluginRoot("toolu"), {})).toEqual([
    process.execPath,
    bundle,
  ]);
  expect(publishedArgv("toolu", "pre-tools", pluginRoot("toolu"), {})).toEqual([bundle]);
  for (const argv of [
    launchedArgv(target, pluginRoot("toolu"), rust),
    entryArgv("toolu", "pre-tools", pluginRoot("toolu"), rust),
    publishedArgv("toolu", "pre-tools", pluginRoot("toolu"), rust),
  ]) {
    expect(argv).toEqual([bin, "hook", "pre-tools"]);
  }
});

test("test names carry the Rust implementation only when it is selected", () => {
  expect(implementationTag("toolu", "pre-tools", {})).toBe("");
  expect(implementationTag("toolu", "pre-tools", { TOOLU_IMPL: "rust:toolu/post-tools" })).toBe("");
  expect(implementationTag("toolu", "pre-tools", { TOOLU_IMPL: "rust:toolu/pre-tools" })).toBe(
    " [rust:toolu/pre-tools]",
  );
});

test("a copied plugin keeps its manifest name; an unselected odd root still runs Bun", async () => {
  using sb = createSandbox();
  const copy = sb.path('plugin "cache"\nfolder');
  cpSync(join(pluginRoot("toolu"), ".claude-plugin"), join(copy, ".claude-plugin"), {
    recursive: true,
  });
  expect(pluginName(copy)).toBe("toolu");
  expect(pluginName(sb.path("bare-plugin"))).toBe("bare-plugin");
  sb.write("broken/.claude-plugin/plugin.json", "{");
  expect(() => pluginName(sb.path("broken"))).toThrow();
  const odd = { plugin: basename(copy), entry: "sample", bundle: SAMPLE };
  const command = resolveEntryCommand(odd, { TOOLU_IMPL: "rust:toolu/pre-tools" });
  expect(command).toEqual({ argv: [process.execPath, SAMPLE], implementation: "bun" });
  expect((await run(command.argv)).exitCode).toBe(0);
});

test("an empty TOOLU_RUST_BIN_DIR is an error, never the current directory", () => {
  expect(() =>
    resolveEntryCommand(ENTRY, { TOOLU_IMPL: "rust:toolu/pre-tools", TOOLU_RUST_BIN_DIR: "" }),
  ).toThrow("TOOLU_RUST_BIN_DIR must not be empty");
});

test("a relative bundle path no longer blocks a selected Rust entry", () => {
  using sb = createSandbox();
  const bin = sb.write("bin/toolu", "#!/bin/sh\nexit 0\n");
  chmodSync(bin, 0o755);
  const relative = { ...ENTRY, bundle: bundlePath("plugins/toolu", "pre-tools") };
  const env = { TOOLU_IMPL: "rust:toolu/pre-tools", TOOLU_RUST_BIN_DIR: sb.path("bin") };
  expect(resolveEntryCommand(relative, env).argv).toEqual([bin, "hook", "pre-tools"]);
});

test("a selected skill CLI entry runs its toolu ledger verb instead of a hook", () => {
  using sb = createSandbox();
  const bin = sb.write("bin/toolu", "#!/bin/sh\nexit 0\n");
  chmodSync(bin, 0o755);
  const env = {
    TOOLU_IMPL: "rust:toolu/plan-ledger,toolu/verdict",
    TOOLU_RUST_BIN_DIR: sb.path("bin"),
  };
  const ledger = {
    plugin: "toolu",
    entry: "plan-ledger",
    bundle: bundlePath(pluginRoot("toolu"), "plan-ledger"),
  };
  expect(resolveEntryCommand(ledger, env).argv).toEqual([bin, "ledger"]);
  const verdict = {
    ...ledger,
    entry: "verdict",
    bundle: bundlePath(pluginRoot("toolu"), "verdict"),
  };
  expect(resolveEntryCommand(verdict, env).argv).toEqual([bin, "ledger", "verdict"]);
  expect([
    isCliEntry("toolu/plan-ledger"),
    isCliEntry("toolu/verdict"),
    isCliEntry("toolu/pre-tools"),
  ]).toEqual([true, true, false]);
});

test("the toolu shim sends ledger commands through the seam and refuses others", async () => {
  using sb = createSandbox();
  const shim = installTooluShim(sb.root, {});
  const selfTest = await run([shim, "ledger", "--self-test"]);
  expect({ code: selfTest.exitCode, out: selfTest.stdout }).toEqual({
    code: 0,
    out: "plan-ledger --self-test: ok\n",
  });
  const verdict = await run([shim, "ledger", "verdict", "table"]);
  expect({ code: verdict.exitCode, err: verdict.stderr }).toEqual({
    code: 2,
    err: "verdict: usage: status | json\n",
  });
  const other = await run([shim, "doctor"]);
  expect({ code: other.exitCode, err: other.stderr }).toEqual({
    code: 127,
    err: "toolu test shim: unsupported command: doctor\n",
  });
});
