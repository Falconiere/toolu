/**
 * SessionStart: publish the statusline at `<config root>/statusline/statusline.sh`,
 * a stable, version-independent path `settings.json` can name (a plugin cannot
 * declare `statusLine` itself). The link is refreshed every session, so plugin
 * updates need no settings change. On Claude, a `statusLine` command that still
 * runs that path through a shell gets a one-line notice to run
 * `/statusline:setup`; this hook never writes `settings.json`.
 */
import { resolve } from "node:path";
import { detectHost } from "@toolu/core/host";
import { publishBunCli } from "@toolu/core/startup";
import { asObject, readObject } from "./statusline/json.ts";
import { isLegacyCommand, settingsPath } from "./statusline/settings.ts";

publishBunCli({
  plugin: "statusline",
  source: resolve(import.meta.dir, "../dist/statusline.js"),
  dir: "statusline",
  name: "statusline.sh",
  tool: "statusline renderer",
});

// publishBunCli already reported an invalid TOOLU_HOST_OVERRIDE.
if (detectHost({ warn: () => {} }) === "claude") {
  const statusLine = asObject(readObject(settingsPath(process.env))?.["statusLine"]);
  if (isLegacyCommand(statusLine?.["command"])) {
    const systemMessage =
      "statusline: settings.json still runs the statusline through a shell, which cannot run the Bun statusline — run /statusline:setup to update it";
    process.stdout.write(`${JSON.stringify({ systemMessage })}\n`);
  }
}
