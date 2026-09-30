#!/usr/bin/env bun
import { CliExit, runCli, writeStdout } from "@toolu/core/cli";
import { parse } from "./jev/parse.ts";
import { project } from "./jev/response.ts";

const URL = "https://api.typesafe.ai/v1/systemone";
const retryable = (status: number) => status === 408 || status === 429 || status >= 500;
const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
function delay(response: Response | undefined, attempt: number): number | null {
  const seconds = response?.headers.get("retry-after");
  const millis = response?.headers.get("retry-after-ms");
  let value = 2 ** (attempt - 1);
  if (seconds !== null && seconds !== undefined && /^\d+$/.test(seconds)) {
    if (seconds.length > 2) return null;
    value = Number(seconds);
  } else if (millis !== null && millis !== undefined && /^\d+$/.test(millis)) {
    if (millis.length > 8) return null;
    value = Math.ceil(Number(millis) / 1000);
  }
  if (!Number.isFinite(value) || value > 60) return null;
  return Math.max(value, 2 ** (attempt - 1)) * 1000;
}
function transportCode(error: unknown): number {
  if (!(error instanceof Error)) return 1;
  if (error.name === "TimeoutError" || error.name === "AbortError") return 28;
  const cause = "cause" in error ? error.cause : undefined;
  const code =
    cause !== null && typeof cause === "object" && "code" in cause ? cause.code : undefined;
  switch (code) {
    case "ENOTFOUND":
      return 6;
    case "ECONNREFUSED":
    case "EHOSTUNREACH":
      return 7;
    case "ETIMEDOUT":
      return 28;
    case "ECONNRESET":
      return 56;
    case "EPIPE":
      return 55;
    default:
      return 1;
  }
}
function timeout(): number {
  const n = Number(process.env.JEV_TIMEOUT ?? "60");
  return Number.isFinite(n) && n >= 0 ? n * 1000 : 60_000;
}
async function post(key: string, body: string): Promise<string> {
  for (let attempt = 1; attempt <= 3; attempt++) {
    let response: Response | undefined;
    try {
      response = await fetch(URL, {
        method: "POST",
        redirect: "manual",
        headers: {
          Authorization: `Bearer ${key}`,
          "Content-Type": "application/json",
          Accept: "application/json",
        },
        body,
        signal: AbortSignal.timeout(timeout()),
      });
      const text = await response.text();
      if (response.status >= 200 && response.status < 300) return text;
      const wait = delay(response, attempt);
      if (attempt < 3 && retryable(response.status) && wait !== null) {
        await pause(wait);
        continue;
      }
      throw new CliExit(22, text);
    } catch (error) {
      if (error instanceof CliExit) throw error;
      if (attempt < 3) {
        await pause(2 ** (attempt - 1) * 1000);
        continue;
      }
      throw new CliExit(
        transportCode(error),
        error instanceof Error ? error.message : String(error),
      );
    }
  }
  throw new CliExit(1, "jev: request failed");
}
async function main(): Promise<number> {
  const key = process.env.TYPESAFE_API_KEY ?? "";
  if (!key) throw new CliExit(1, "jev: TYPESAFE_API_KEY unset");
  if (/[\r\n]/.test(key))
    throw new CliExit(1, "jev: TYPESAFE_API_KEY must not contain line breaks");
  const call = await parse(process.argv.slice(2));
  const body = await post(
    key,
    JSON.stringify({ state: call.state, model: call.model, questions: call.questions }),
  );
  await writeStdout(project(call, body));
  return 0;
}
await runCli(main);
