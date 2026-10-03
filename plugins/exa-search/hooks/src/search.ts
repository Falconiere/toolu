#!/usr/bin/env -S bun --no-env-file
/**
 * Exa API CLI — search, crawl, and find similar content. The TypeScript port of
 * the bash search.sh (#270): same commands, flags, output and exit statuses.
 * Reads EXA_API_KEY from the environment (never from .env).
 */
import { CliExit, runCli, writeStdout } from "@toolu/core/cli";
import { jsonOutput, send } from "@toolu/core/rest";
import { leanResponse } from "./exa/lean.ts";
import { crawlCall, type ExaCall, searchCall, similarCall } from "./exa/requests.ts";
import { MAIN_USAGE } from "./exa/usage.ts";

const TOOL = "exa-search";
const EXA_URL = "https://api.exa.ai";

function callFor(argv: readonly string[]): ExaCall {
  const [command = "", ...rest] = argv;
  switch (command) {
    case "":
    case "-h":
    case "--help":
      throw new CliExit(1, MAIN_USAGE);
    case "search":
      return searchCall(rest);
    case "crawl":
      return crawlCall(rest);
    case "similar":
      return similarCall(rest);
    default:
      // A bare query defaults to search.
      return searchCall(argv);
  }
}

async function main(): Promise<number> {
  const key = process.env["EXA_API_KEY"] ?? "";
  if (key === "") throw new CliExit(1, `${TOOL}: EXA_API_KEY unset`);
  const call = callFor(process.argv.slice(2));
  const text = await send(TOOL, {
    url: `${EXA_URL}/${call.endpoint}`,
    method: "POST",
    headers: { "x-api-key": key, "Content-Type": "application/json", Accept: "application/json" },
    body: call.body,
  });
  await writeStdout(call.lean ? jsonOutput(TOOL, text, leanResponse) : jsonOutput(TOOL, text));
  return 0;
}

await runCli(main);
