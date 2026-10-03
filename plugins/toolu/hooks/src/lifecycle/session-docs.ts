/**
 * The doc part of toolu's SessionStart context (#263): the Session Protocol
 * with the event's extras, the model-routing table, and the opt-in
 * per-toolchain notes, rendered from `hooks/docs/*.md` like session-start.sh.
 */
import { join } from "node:path";
import { MODEL_CLASSES, codexModel, enabled, model, type LoadedConfig } from "@toolu/core/config";
import type { HostName } from "@toolu/core/host";
import { renderDoc } from "./render-doc.ts";
import type { ProjectFacts } from "./project-detect.ts";

const TITLES: Readonly<Record<string, string>> = {
  startup: "Toolu is on!",
  resume: "Session resumed",
  clear: "Context cleared",
  compact: "Context compacted",
};

/** The `systemMessage` title; "" for an event session-start.sh had no branch for. */
export function eventTitle(event: string): string {
  return TITLES[event] ?? "";
}

type Token = readonly [string, string];

/** `{{model_*}}` then `{{effort_*}}`, resolved once from config for this host. */
function routingTokens(config: LoadedConfig, host: HostName): Token[] {
  const models: Token[] = [];
  const efforts: Token[] = [];
  for (const cls of MODEL_CLASSES) {
    if (host === "codex") {
      const picked = codexModel(config, cls);
      models.push([`model_${cls}`, picked.model]);
      efforts.push([`effort_${cls}`, `, effort \`${picked.reasoningEffort}\``]);
    } else {
      models.push([`model_${cls}`, model(config, cls)]);
      efforts.push([`effort_${cls}`, ""]);
    }
  }
  return [...models, ...efforts];
}

export type DocInput = {
  docs: string;
  event: string;
  config: LoadedConfig;
  host: HostName;
  facts: ProjectFacts;
  verbose: boolean;
};

/** The doc parts in session-start.sh's order; missing docs are skipped. */
export function docParts(input: DocInput): string[] {
  const { docs, facts } = input;
  const tokens: Token[] = [
    ["project_name", facts.name === "" ? "this project" : facts.name],
    ["node_pm", facts.nodePm === "" ? "your package manager" : facts.nodePm],
    ...routingTokens(input.config, input.host),
  ];
  const render = (file: string): string => renderDoc(join(docs, file), tokens);
  const parts: string[] = [];
  if (input.event === "compact") parts.push(renderDoc(join(docs, "post-compaction.md"), []));
  if (eventTitle(input.event) !== "") parts.push(render("session-start.md"));
  if (enabled(input.config, "models", "enabled")) {
    parts.push(
      render(input.host === "opencode" ? "model-routing-opencode.md" : "model-routing.md"),
    );
  }
  if (input.verbose) {
    if (facts.ts) parts.push(render("session-start-ts.md"));
    if (facts.rust) parts.push(render("session-start-rust.md"));
    if (facts.python) parts.push(render("session-start-python.md"));
  }
  return parts.filter((part) => part !== "");
}
