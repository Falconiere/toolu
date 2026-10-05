import { expect, test } from "bun:test";
import { chmodSync, existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { DEFAULT_MANIFEST, PortsError, portSelector, readPorted } from "../rust-conformance.ts";

const SCRIPT = resolve(import.meta.dir, "../rust-conformance.ts");

test.concurrent("the committed port list is the sole, valid manifest", () => {
  const entries = readPorted(DEFAULT_MANIFEST);
  expect(() => portSelector(entries)).not.toThrow();
});

test.concurrent("an empty list is a successful no-op that never calls cargo", async () => {
  using sb = createSandbox();
  const manifest = sb.write("ports.json", '{ "entries": [] }\n');
  const calls = sb.path("cargo-calls.txt");
  const cargo = sb.write("cargo", '#!/bin/sh\necho "$*" >> "$CALLS"\nexit 0\n');
  chmodSync(cargo, 0o755);
  const result = await run([process.execPath, SCRIPT, "--manifest", manifest], {
    env: { CARGO: cargo, CALLS: calls },
  });
  expect(result.exitCode).toBe(0);
  expect(result.stdout).toBe("rust-conformance: no ported entries; nothing to run\n");
  expect(existsSync(calls)).toBe(false);
});

test.concurrent("malformed and duplicate lists are rejected before cargo runs", async () => {
  using sb = createSandbox();
  const calls = sb.path("cargo-calls.txt");
  const cargo = sb.write("cargo", '#!/bin/sh\necho "$*" >> "$CALLS"\nexit 0\n');
  chmodSync(cargo, 0o755);
  const cases: [string, string][] = [
    ["not-json.json", "{"],
    ["wrong-shape.json", '{ "ports": [] }'],
    ["extra-key.json", '{ "entries": [], "note": "x" }'],
    ["bad-entry.json", '{ "entries": ["toolu"] }'],
    ["traversal.json", '{ "entries": ["toolu/../pre-tools"] }'],
    ["duplicate.json", '{ "entries": ["toolu/pre-tools", "toolu/pre-tools"] }'],
  ];
  const results = await Promise.all(
    cases.map(async ([name, body]) => {
      const result = await run([process.execPath, SCRIPT, "--manifest", sb.write(name, body)], {
        env: { CARGO: cargo, CALLS: calls },
      });
      return {
        name,
        exitCode: result.exitCode,
        prefixed: result.stderr.startsWith("rust-conformance: "),
      };
    }),
  );
  expect(results).toEqual(cases.map(([name]) => ({ name, exitCode: 2, prefixed: true })));
  expect(existsSync(calls)).toBe(false);
});

test.concurrent("a populated list builds the toolu binary and stops when the build fails", async () => {
  using sb = createSandbox();
  const manifest = sb.write(
    "ports.json",
    '{ "entries": ["toolu/pre-tools", "jev/session-start"] }',
  );
  const calls = sb.path("cargo-calls.txt");
  const cargo = sb.write("cargo", '#!/bin/sh\necho "$*" >> "$CALLS"\nexit 101\n');
  chmodSync(cargo, 0o755);
  const result = await run([process.execPath, SCRIPT, "--manifest", manifest], {
    env: { CARGO: cargo, CALLS: calls },
  });
  expect(result.exitCode).toBe(1);
  expect(result.stdout).toBe(
    "rust-conformance: TOOLU_IMPL=rust:toolu/pre-tools,jev/session-start\n",
  );
  expect(result.stderr).toContain("cargo build of the toolu binary failed");
  expect(readFileSync(calls, "utf8")).toBe("build --release --locked --bin toolu\n");
});

test.concurrent("the selector is the exact entry list", () => {
  expect(portSelector([])).toBeNull();
  expect(portSelector(["toolu/pre-tools"])).toBe("rust:toolu/pre-tools");
  expect(() => portSelector(["toolu/pre-tools", "toolu/pre-tools"])).toThrow(PortsError);
});
