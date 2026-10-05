/** Shared PostToolUse corpus loaded from declarative, host-neutral JSON. */
import { resolve } from "node:path";
import { z } from "zod";
import { bashFixture, postToolFixture, toStdin, type Fixture } from "./fixtures.ts";
import {
  applyCaseSetup,
  HostSetupSchema,
  materializeToolFixture,
  readCaseFile,
} from "./json-cases.ts";
import { installPlugins, type PretoolHost } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";

const OUTCOME = z.enum(["block", "advisory", "silent", "exit2"]);
const CASE = z.strictObject({
  name: z.string().min(1),
  fixture: z.unknown().optional(),
  stdin: z.string().optional(),
  config: z.record(z.string(), z.json()).optional(),
  setupByHost: HostSetupSchema,
  expect: OUTCOME,
});

export type PostOutcome = z.infer<typeof OUTCOME>;
export type PosttoolCase = z.infer<typeof CASE>;

const CORPUS_PATH = resolve(import.meta.dir, "../../../../fixtures/gates/posttool-corpus.json");
export const POSTTOOL_CORPUS: readonly PosttoolCase[] = readCaseFile(CORPUS_PATH).map((item) =>
  CASE.parse(item),
);

/** A Bash call that already ran with exit status `code`. */
export function ran(command: string, code: number): (sb: Sandbox) => Fixture {
  return () =>
    postToolFixture(bashFixture(command), {
      metadata: { exit_code: code },
      stdout: "",
      stderr: "",
    });
}

const SPECS = ["toolu@toolu", "fixture@toolu", "ts-quality@toolu"];

/** Put the sandbox in this host's starting state. */
export function preparePost(sb: Sandbox, host: PretoolHost, c: PosttoolCase): void {
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  installPlugins(sb, ...SPECS);
  if (c.config !== undefined) sb.writeConfig(host, "project", c.config);
  applyCaseSetup(sb, c.setupByHost[host], host);
}

/** Encode a committed fixture through the host's actual hook envelope. */
export function postStdin(sb: Sandbox, host: PretoolHost, c: PosttoolCase): string {
  if (c.stdin !== undefined) return c.stdin;
  if (c.fixture === undefined) throw new Error(`${c.name}: neither a fixture nor stdin`);
  return JSON.stringify(
    toStdin(host, materializeToolFixture(sb, c.fixture, host), { cwd: sb.project }),
  );
}
