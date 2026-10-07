/**
 * The TypeScript dispatcher as a process, for the Rust engine's live A/B test
 * (`crates/core/engine/tests/quality_bridge.rs`, #418): one JSON request on
 * stdin (`phase`, `stdin`, `env`, `cwd`, `libDir`), one JSON result on stdout
 * (`stdout`, `stderr`, `exitCode`). No built-ins: the registry alone runs.
 */
import { z } from "zod";
import { dispatchPostTool, dispatchPreTool } from "../dispatch.ts";

const RequestSchema = z.strictObject({
  phase: z.enum(["pre", "post"]),
  stdin: z.string(),
  env: z.record(z.string(), z.string()),
  cwd: z.string(),
  libDir: z.string(),
});

const request = RequestSchema.parse(JSON.parse(await Bun.stdin.text()));
const options = { builtins: [], libDir: request.libDir, env: request.env, cwd: request.cwd };
const result =
  request.phase === "pre"
    ? await dispatchPreTool(request.stdin, options)
    : await dispatchPostTool(request.stdin, options);
process.stdout.write(`${JSON.stringify(result)}\n`);
