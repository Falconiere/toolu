import { z } from "zod";

export const HOSTS = ["claude", "codex", "opencode"] as const;
export const SCOPES = ["user", "project", "local"] as const;

export const VERBS = ["install", "list", "remove", "update"] as const;

export const hostSchema = z.enum(HOSTS);
export const scopeSchema = z.enum(SCOPES);
export const verbSchema = z.enum(VERBS);

export type Host = z.infer<typeof hostSchema>;
export type Scope = z.infer<typeof scopeSchema>;
export type Verb = z.infer<typeof verbSchema>;

/** Every flag the CLI accepts, already normalized. */
export interface ParsedArgs {
  readonly verb: Verb | undefined;
  readonly names: readonly string[];
  readonly host: Host | undefined;
  readonly scope: Scope | undefined;
  readonly config: string | undefined;
  readonly yes: boolean;
  readonly force: boolean;
  readonly dryRun: boolean;
  readonly noInput: boolean;
  readonly json: boolean;
  readonly help: boolean;
  readonly version: boolean;
}
