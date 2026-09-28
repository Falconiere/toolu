#!/usr/bin/env bun
// @bun

// packages/toolu-core/src/cli/cli.ts
class CliExit extends Error {
  code;
  stdout;
  constructor(code, message = "", stdout = "") {
    super(message);
    this.name = "CliExit";
    this.code = code;
    this.stdout = stdout;
  }
}
async function writeStdout(text) {
  if (text !== "")
    await Bun.write(Bun.stdout, text);
}
async function writeStderr(text) {
  if (text !== "")
    await Bun.write(Bun.stderr, text.endsWith(`
`) ? text : `${text}
`);
}
async function runCli(main) {
  let code;
  try {
    code = await main();
  } catch (error) {
    if (error instanceof CliExit) {
      await writeStdout(error.stdout);
      await writeStderr(error.message);
      code = error.code;
    } else {
      await writeStderr(error instanceof Error ? error.message : String(error));
      code = 1;
    }
  }
  process.exit(code);
}

// plugins/agent-browser/hooks/src/agent-browser.ts
import { constants } from "os";

// plugins/agent-browser/hooks/src/shaping/shaping.ts
var DEFAULT_MAX_OUTPUT = "4000";
function hasFlag(needle, args) {
  return args.some((arg) => arg === needle || arg.startsWith(`${needle}=`));
}
function injectedFlags(command, args) {
  const inject = [];
  const add = (flag, ...value) => {
    if (!hasFlag(flag, args))
      inject.push(flag, ...value);
  };
  switch (command) {
    case "snapshot":
      if (!["-i", "-a", "-s", "-d"].some((scope) => hasFlag(scope, args)))
        inject.push("-i");
      add("--json");
      add("--max-output", DEFAULT_MAX_OUTPUT);
      add("--content-boundaries");
      break;
    case "get":
      add("--json");
      add("--max-output", DEFAULT_MAX_OUTPUT);
      add("--content-boundaries");
      break;
    case "find":
    case "diff":
      add("--json");
      add("--max-output", DEFAULT_MAX_OUTPUT);
      break;
    default:
      break;
  }
  return inject;
}

// plugins/agent-browser/hooks/src/agent-browser.ts
var INSTALL_GUIDE = "agent-browser not found \u2014 install: npm i -g agent-browser && agent-browser install  (or: brew install agent-browser / cargo install agent-browser)";
var FORWARDED = ["SIGINT", "SIGTERM", "SIGHUP"];
function binaryArgv(args) {
  const [first, ...rest] = args;
  if (first === undefined)
    return [];
  if (first === "--raw")
    return rest;
  return [first, ...injectedFlags(first, rest), ...rest];
}
async function run(bin, argv) {
  const child = Bun.spawn([bin, ...argv], {
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit"
  });
  const forward = (signal) => {
    child.kill(signal);
  };
  for (const signal of FORWARDED)
    process.on(signal, forward);
  const code = await child.exited;
  for (const signal of FORWARDED)
    process.off(signal, forward);
  const signal = child.signalCode;
  return signal === null ? code : 128 + constants.signals[signal];
}
async function main() {
  const override = process.env["AGENT_BROWSER_BIN"] ?? "";
  const name = override === "" ? "agent-browser" : override;
  const bin = Bun.which(name);
  if (bin === null)
    throw new CliExit(127, INSTALL_GUIDE);
  return run(bin, binaryArgv(process.argv.slice(2)));
}
await runCli(main);
