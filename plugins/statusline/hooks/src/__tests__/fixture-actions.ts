/** Bounded real-sandbox actions shared by the statusline parity consumers. */
import { chmodSync, copyFileSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { materializeCaseValue } from "@toolu/conformance/harness/json-cases";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { JEV_BUNDLE, git, payload, publishJev, put, repo, withRemote } from "./harness.ts";

export const FixtureActionSchema = z.discriminatedUnion("op", [
  z.strictObject({ op: z.literal("repo"), path: z.json() }),
  z.strictObject({ op: z.literal("remote"), branch: z.string() }),
  z.strictObject({ op: z.literal("write"), path: z.json(), body: z.string() }),
  z.strictObject({ op: z.literal("mkdir"), path: z.json() }),
  z.strictObject({ op: z.literal("git"), cwd: z.json(), args: z.array(z.json()) }),
  z.strictObject({
    op: z.literal("publish-jev"),
    root: z.json(),
    host: z.enum(["claude", "codex"]),
    env: z.record(z.string(), z.json()).optional(),
  }),
  z.strictObject({ op: z.literal("remove"), path: z.json() }),
  z.strictObject({ op: z.literal("symlink"), path: z.json(), target: z.json() }),
  z.strictObject({ op: z.literal("copy"), path: z.json(), from: z.enum(["jev-bundle"]) }),
  z.strictObject({ op: z.literal("chmod"), path: z.json(), mode: z.number().int() }),
  z.strictObject({ op: z.literal("link-tool"), path: z.json(), tool: z.literal("git") }),
]);

export const FixturePayloadSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("ctx"), tokens: z.number(), pct: z.number() }),
  z.strictObject({ kind: z.literal("workspace"), cwd: z.json(), extra: z.string() }),
  z.strictObject({ kind: z.literal("raw"), value: z.string() }),
  z.strictObject({ kind: z.literal("object"), value: z.json() }),
]);

export function fixtureString(sb: Sandbox, value: unknown): string {
  return z.string().parse(materializeCaseValue(sb, value));
}

export async function applyFixtureActions(
  sb: Sandbox,
  actions: readonly z.infer<typeof FixtureActionSchema>[],
): Promise<void> {
  for (const action of actions) {
    if (action.op === "repo") repo(fixtureString(sb, action.path));
    else if (action.op === "remote") withRemote(sb, action.branch);
    else if (action.op === "write") put(fixtureString(sb, action.path), action.body);
    else if (action.op === "mkdir") mkdirSync(fixtureString(sb, action.path), { recursive: true });
    else if (action.op === "git")
      git(fixtureString(sb, action.cwd), ...action.args.map((arg) => fixtureString(sb, arg)));
    else if (action.op === "publish-jev") {
      const env = Object.fromEntries(
        Object.entries(action.env ?? {}).map(([key, value]) => [key, fixtureString(sb, value)]),
      );
      await publishJev(fixtureString(sb, action.root), action.host, env);
    } else if (action.op === "remove")
      rmSync(fixtureString(sb, action.path), { recursive: true, force: true });
    else if (action.op === "symlink")
      symlinkSync(fixtureString(sb, action.target), fixtureString(sb, action.path));
    else if (action.op === "copy") copyFileSync(JEV_BUNDLE, fixtureString(sb, action.path));
    else if (action.op === "chmod") chmodSync(fixtureString(sb, action.path), action.mode);
    else {
      const found = Bun.which(action.tool);
      if (found !== null) symlinkSync(found, fixtureString(sb, action.path));
    }
  }
}

export function fixturePayload(
  sb: Sandbox,
  value: z.infer<typeof FixturePayloadSchema> | undefined,
): string | undefined {
  if (value === undefined) return undefined;
  if (value.kind === "ctx")
    return `{"model":{"display_name":"Opus"},"context_window":{"context_window_size":200000,"total_input_tokens":${String(value.tokens)},"used_percentage":${String(value.pct)}}}`;
  if (value.kind === "workspace") return payload(fixtureString(sb, value.cwd), value.extra);
  if (value.kind === "raw") return value.value;
  return JSON.stringify(materializeCaseValue(sb, value.value));
}
