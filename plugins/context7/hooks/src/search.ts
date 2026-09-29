#!/usr/bin/env bun
/**
 * Context7 CLI — search libraries and query documentation. The TypeScript port
 * of the bash search.sh (#270): same commands, flags, output and exit statuses.
 * Reads CONTEXT7_API_KEY from the environment (never from .env); only a key
 * starting with `ctx7sk` is sent, as a Bearer token.
 */
import { CliExit, runCli, writeStdout } from "@toolu/core/cli";
import { encodeQuery, jsonOutput, send } from "@toolu/core/rest";
import { type Context7Call, docsCall, searchCall } from "./context7/requests.ts";
import { MAIN_USAGE } from "./context7/usage.ts";

const TOOL = "context7";
const C7_URL = "https://context7.com/api/v2";

function callFor(argv: readonly string[]): Context7Call {
  const [command = "", ...rest] = argv;
  switch (command) {
    case "":
    case "-h":
    case "--help":
      throw new CliExit(1, MAIN_USAGE);
    case "search":
      return searchCall(rest);
    case "docs":
      return docsCall(rest);
    default:
      // Bare arguments default to search.
      return searchCall(argv);
  }
}

async function main(): Promise<number> {
  const call = callFor(process.argv.slice(2));
  const headers: Record<string, string> = { Accept: "application/json" };
  const key = process.env["CONTEXT7_API_KEY"] ?? "";
  if (key.startsWith("ctx7sk")) headers["Authorization"] = `Bearer ${key}`;
  const text = await send(TOOL, {
    url: `${C7_URL}/${call.endpoint}${encodeQuery(call.params)}`,
    headers,
    json: call.json,
  });
  await writeStdout(call.json ? jsonOutput(TOOL, text) : text);
  return 0;
}

await runCli(main);
