/**
 * Every hook entry in the repository (#410): each distinct `<plugin>/<entry>`
 * whose `hooks.json` command launches `hooks/dist/<entry>.js`, with the event it
 * is first registered under and the exact launcher command hosts run.
 */
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { pluginName } from "@toolu/conformance/harness/entry-command";
import { z } from "zod";
import { loadJson } from "./hook-data.ts";

export type HookEntry = {
  /** `<plugin>/<entry>`, the `TOOLU_IMPL` and budget key. */
  readonly id: string;
  readonly plugin: string;
  readonly entry: string;
  readonly event: string;
  /** The plugin directory, exported to the hook as `CLAUDE_PLUGIN_ROOT`. */
  readonly root: string;
  /** The `hooks.json` launcher command, run under `/bin/sh -c`. */
  readonly command: string;
};

const HooksFile = z.looseObject({
  hooks: z.record(
    z.string(),
    z.array(z.looseObject({ hooks: z.array(z.looseObject({ command: z.string() })) })),
  ),
});

const BUNDLE = /\/hooks\/dist\/([a-z0-9][a-z0-9-]*)\.js/;

function entriesOf(root: string): HookEntry[] {
  const file = join(root, "hooks", "hooks.json");
  if (!existsSync(file)) return [];
  const plugin = pluginName(root);
  const parsed = loadJson(file, HooksFile);
  return Object.entries(parsed.hooks).flatMap(([event, groups]) =>
    groups.flatMap((group) =>
      group.hooks.flatMap(({ command }) => {
        const entry = BUNDLE.exec(command)?.[1];
        return entry === undefined
          ? []
          : [{ id: `${plugin}/${entry}`, plugin, entry, event, root, command }];
      }),
    ),
  );
}

/** The hook entries under `pluginsDir`, one per id (first registration wins), sorted by id. */
export function discoverEntries(pluginsDir: string): HookEntry[] {
  const byId = new Map<string, HookEntry>();
  const dirs = readdirSync(pluginsDir, { withFileTypes: true })
    .filter((dirent) => dirent.isDirectory())
    .map((dirent) => join(pluginsDir, dirent.name))
    .toSorted();
  for (const found of dirs.flatMap(entriesOf)) {
    if (!byId.has(found.id)) byId.set(found.id, found);
  }
  return [...byId.values()].toSorted((a, b) => a.id.localeCompare(b.id));
}
