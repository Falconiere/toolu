/** The pinned parser's raw JSON result, before toolu's shell analysis walks it. */
import { parse } from "unbash";

export function parseUnbash(source: string): unknown {
  try {
    return JSON.parse(JSON.stringify(parse(source)));
  } catch (error) {
    return {
      type: "ParserException",
      message: error instanceof Error ? error.message : String(error),
    };
  }
}
