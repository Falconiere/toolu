/**
 * Registry module contract (#257). A plugin contributes one bundled ESM file per
 * event to `<config>/toolu/<dir>.d/<spec>__<name>.js`; its default export is a
 * `RegistryModule` that the core dispatcher imports in-process.
 */
import type { Decision } from "../decision/decision.ts";
import type { NormalizedEvent } from "../events/events.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";
import type { EditRecord } from "../state/state-schema.ts";

/** Canonical events a registry module can subscribe to; `tool/pre` modules also see `shell/pre`. */
export const REGISTRY_EVENTS = ["tool/pre", "tool/post"] as const;

export type RegistryEvent = (typeof REGISTRY_EVENTS)[number];

/** What `run` receives: the normalized tool event the dispatcher is handling. */
export type RegistryHookEvent = Extract<
  NormalizedEvent,
  { type: "tool/pre" | "tool/post" | "shell/pre" }
>;

/**
 * Per-event context. An interface on purpose: later layers add optional fields
 * (the lazily parsed shell command from #284) without breaking a module.
 */
export interface RegistryContext {
  readonly host: HostName;
  readonly env: HostEnv;
  readonly configRoot: string;
  readonly projectRoot: string;
  /** The host's raw hook payload, for ports that must match bash byte for byte. */
  readonly raw: Readonly<Record<string, unknown>>;
  /** Set when the dispatcher split a multi-file patch into one payload per path. */
  readonly edit?: {
    readonly operation: EditRecord["operation"];
    readonly from: string;
    readonly movedTo: string;
  };
}

export interface RegistryModule {
  /** Owning plugin, `name@marketplace`; must equal the `<spec>` of the file name. */
  readonly spec: string;
  /** Must equal the `<name>` of the file name. */
  readonly name: string;
  /** Must match the directory the file sits in. */
  readonly event: RegistryEvent;
  run(event: RegistryHookEvent, ctx: RegistryContext): Promise<Decision>;
}

/** Identity helper that type-checks a module's default export. */
export function defineRegistryModule<T extends RegistryModule>(module: T): T {
  return module;
}

/** The registry directory a hook event is dispatched from. */
export function registryEventFor(type: RegistryHookEvent["type"]): RegistryEvent {
  return type === "tool/post" ? "tool/post" : "tool/pre";
}
