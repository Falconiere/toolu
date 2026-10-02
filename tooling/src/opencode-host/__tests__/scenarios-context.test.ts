import { expect, test } from "bun:test";
import { summarize } from "../scenarios-context.ts";
import { ContractError } from "../schema.ts";

// A real loopback server stands in for `opencode serve` answering with an error (#335).

test.concurrent("a failing GET /session from opencode serve is a contract error naming the status", async () => {
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch: () => new Response("boom", { status: 500 }),
  });
  try {
    const err = await summarize(`http://127.0.0.1:${server.port}`, "/tmp/project").then(
      () => null,
      (error: unknown) => error,
    );
    expect(err).toBeInstanceOf(ContractError);
    expect(err instanceof Error ? err.message : "").toBe(
      "opencode serve GET /session failed (500)",
    );
  } finally {
    void server.stop(true);
  }
});
