/**
 * SessionStart (#269): publish the Exa search CLI at
 * `<config root>/exa-search/search.sh`, a stable path the agent's shell can expand
 * (`CLAUDE_PLUGIN_ROOT` reaches hook processes only). OpenCode receives
 * project-path and credential guidance on success; other hosts stay silent.
 * Every failure is non-fatal. Port of the bash `session-start.sh`.
 */
import { resolve } from "node:path";
import { publishBunCli, renderHookOutput, sessionContext } from "@toolu/core/startup";

const published = publishBunCli({
  plugin: "exa-search",
  source: resolve(import.meta.dir, "../dist/search.js"),
  dir: "exa-search",
  name: "search.sh",
  tool: "exa-search search CLI",
});

if (process.env.TOOLU_HOST_OVERRIDE === "opencode" && published.status === "published") {
  const guidance =
    (process.env.EXA_API_KEY ?? "") === ""
      ? "EXA_API_KEY is unset. For web search, use OpenCode's websearch if available; for a known URL, use webfetch. Do not call the Exa helper until the key is set."
      : "Load the native skill exa-search-exa-search for the research workflow. Use search for web queries, crawl for known URLs, and similar for related pages.";
  const context = sessionContext(
    "SessionStart",
    `exa-search helper: ${published.path}. ${guidance} Native skill: exa-search-exa-search.`,
  );
  if (context !== undefined) process.stdout.write(renderHookOutput(context, false));
}
