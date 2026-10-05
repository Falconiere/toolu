/**
 * SessionStart (#269): publish the Exa search CLI at
 * `<config root>/exa-search/search.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). OpenCode receives
 * project-path and credential guidance on success. Every host gets the
 * deprecation notice (#403), on OpenCode once per start, not per compaction.
 * Every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import {
  deprecatedStartupOutput,
  publishBunCli,
  sessionContext,
  startedByCompaction,
  type SessionContext,
} from "@toolu/core/startup";

const published = publishBunCli({
  plugin: "exa-search",
  source: resolve(import.meta.dir, "../dist/search.js"),
  dir: "exa-search",
  name: "search.sh",
  tool: "exa-search search CLI",
});

const onOpencode = process.env.TOOLU_HOST_OVERRIDE === "opencode";
let context: SessionContext | undefined;
if (onOpencode && published.status === "published") {
  const guidance =
    (process.env.EXA_API_KEY ?? "") === ""
      ? "EXA_API_KEY is unset. For web search, use OpenCode's websearch if available; for a known URL, use webfetch. Do not call the Exa helper until the key is set."
      : "Load the native skill exa-search-exa-search for the research workflow. Use search for web queries, crawl for known URLs, and similar for related pages.";
  context = sessionContext(
    "SessionStart",
    `exa-search helper: ${published.path}. ${guidance} Native skill: exa-search-exa-search.`,
  );
}
const compacting = onOpencode && (await startedByCompaction());
process.stdout.write(deprecatedStartupOutput("exa-search", context, { compacting }));
