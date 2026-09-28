#!/usr/bin/env bun
/**
 * agent-browser wrapper — bakes token-lean defaults onto the read-heavy command
 * family so the agent spends context on the accessibility tree, not raw HTML or
 * screenshots. The TypeScript port of the bash agent-browser.sh (#270).
 *
 * A leading `--raw` bypasses all shaping. The binary is never auto-installed —
 * a missing one prints an install guide and exits 127. $AGENT_BROWSER_BIN
 * overrides the binary (absolute path or PATH name), for a custom build or tests.
 */
import { CliExit, runCli } from "@toolu/core/cli";
import { constants } from "node:os";
import { injectedFlags } from "./shaping/shaping.ts";

const INSTALL_GUIDE =
  "agent-browser not found — install: npm i -g agent-browser && agent-browser install  (or: brew install agent-browser / cargo install agent-browser)";

/** Signals a terminal or supervisor sends the wrapper; the child gets them too, as under exec. */
const FORWARDED = ["SIGINT", "SIGTERM", "SIGHUP"] as const;

function binaryArgv(args: readonly string[]): string[] {
  const [first, ...rest] = args;
  if (first === undefined) return [];
  if (first === "--raw") return rest;
  return [first, ...injectedFlags(first, rest), ...rest];
}

/** Runs the binary with inherited stdio and returns its status the way a shell would. */
async function run(bin: string, argv: readonly string[]): Promise<number> {
  const child = Bun.spawn([bin, ...argv], {
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  });
  const forward = (signal: NodeJS.Signals): void => {
    child.kill(signal);
  };
  for (const signal of FORWARDED) process.on(signal, forward);
  const code = await child.exited;
  for (const signal of FORWARDED) process.off(signal, forward);
  const signal = child.signalCode;
  return signal === null ? code : 128 + constants.signals[signal];
}

async function main(): Promise<number> {
  // Unset or empty falls back, as bash's ${AGENT_BROWSER_BIN:-agent-browser} did.
  const override = process.env["AGENT_BROWSER_BIN"] ?? "";
  const name = override === "" ? "agent-browser" : override;
  const bin = Bun.which(name);
  if (bin === null) throw new CliExit(127, INSTALL_GUIDE);
  return run(bin, binaryArgv(process.argv.slice(2)));
}

await runCli(main);
