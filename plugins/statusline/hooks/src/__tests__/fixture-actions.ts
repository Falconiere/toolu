/** Bounded real-sandbox actions shared by the statusline parity consumers. */
import { chmodSync, copyFileSync, mkdirSync, rmSync, symlinkSync } from "node:fs";
import { materializeCaseValue, mutationPath } from "@toolu/conformance/harness/json-cases";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { JEV_BUNDLE, git, payload, publishJev, put, repo, withRemote } from "./harness.ts";

const TaggedPathSchema = z.strictObject({ $path: z.string() });

export const FixtureActionSchema = z.discriminatedUnion("op", [
  z.strictObject({ op: z.literal("repo"), path: TaggedPathSchema }),
  z.strictObject({ op: z.literal("remote"), branch: z.string() }),
  z.strictObject({ op: z.literal("write"), path: TaggedPathSchema, body: z.string() }),
  z.strictObject({ op: z.literal("mkdir"), path: TaggedPathSchema }),
  z.strictObject({ op: z.literal("git"), cwd: TaggedPathSchema, args: z.array(z.json()) }),
  z.strictObject({
    op: z.literal("publish-jev"),
    root: TaggedPathSchema,
    host: z.enum(["claude", "codex"]),
    env: z.record(z.string(), z.json()).optional(),
  }),
  z.strictObject({ op: z.literal("remove"), path: TaggedPathSchema }),
  z.strictObject({ op: z.literal("symlink"), path: TaggedPathSchema, target: TaggedPathSchema }),
  z.strictObject({ op: z.literal("copy"), path: TaggedPathSchema, from: z.enum(["jev-bundle"]) }),
  z.strictObject({ op: z.literal("chmod"), path: TaggedPathSchema, mode: z.number().int() }),
  z.strictObject({ op: z.literal("link-tool"), path: TaggedPathSchema, tool: z.literal("git") }),
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
    if (action.op === "repo") repo(mutationPath(sb, action.path.$path));
    else if (action.op === "remote") withRemote(sb, action.branch);
    else if (action.op === "write") put(mutationPath(sb, action.path.$path), action.body);
    else if (action.op === "mkdir")
      mkdirSync(mutationPath(sb, action.path.$path), { recursive: true });
    else if (action.op === "git")
      git(mutationPath(sb, action.cwd.$path), ...action.args.map((arg) => fixtureString(sb, arg)));
    else if (action.op === "publish-jev") {
      const env = Object.fromEntries(
        Object.entries(action.env ?? {}).map(([key, value]) => [key, fixtureString(sb, value)]),
      );
      await publishJev(mutationPath(sb, action.root.$path), action.host, env);
    } else if (action.op === "remove")
      rmSync(mutationPath(sb, action.path.$path, "claude", true), { recursive: true, force: true });
    else if (action.op === "symlink")
      symlinkSync(fixtureString(sb, action.target), mutationPath(sb, action.path.$path));
    else if (action.op === "copy") copyFileSync(JEV_BUNDLE, mutationPath(sb, action.path.$path));
    else if (action.op === "chmod") chmodSync(mutationPath(sb, action.path.$path), action.mode);
    else {
      const found = Bun.which(action.tool);
      if (found !== null) symlinkSync(found, mutationPath(sb, action.path.$path));
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
