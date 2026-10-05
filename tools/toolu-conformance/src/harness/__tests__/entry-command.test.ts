import { expect, test } from "bun:test";
import { chmodSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "../sandbox.ts";
import { run } from "../spawn.ts";
import { resolveEntryCommand } from "../entry-command.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
const SAMPLE = join(ROOT, "plugins/toolu/hooks/dist/sample.js");
const ENTRY = { plugin: "toolu", entry: "pre-tools", bundle: SAMPLE };

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
