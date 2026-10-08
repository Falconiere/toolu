import { expect, test } from "bun:test";
import { lstatSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { builtTooluBinary, bundlePath, entryArgv } from "@toolu/conformance/harness/entry-command";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";

// Cross-plugin guard for the host-native roots used by SessionStart publishers.

const ROOT = resolve(import.meta.dir, "../../..");

type Publisher = { plugin: string; published: string };

const SESSION_START: readonly Publisher[] = [
  { plugin: "jev", published: "jev/jev.sh" },
  { plugin: "statusline", published: "statusline/statusline.sh" },
  { plugin: "toolu-review", published: "toolu-review/write-state.sh" },
];

const REGISTRY: readonly Publisher[] = [
  { plugin: "ast-grep", published: "pre-tools.d/ast-grep@toolu__search-nudge.js" },
  {
    plugin: "python-quality",
    published: "post-tools.d/python-quality@toolu__python-quality.js",
  },
  { plugin: "rust-quality", published: "post-tools.d/rust-quality@toolu__rust-quality.js" },
  { plugin: "ts-quality", published: "post-tools.d/ts-quality@toolu__ts-quality.js" },
];

/** Run one plugin hook the way Codex does: HOME isolated, CODEX_HOME per plugin, no Claude root. */
async function codexHook(
  sb: Sandbox,
  item: Publisher,
  script: string,
): Promise<{ exitCode: number; codexHome: string }> {
  const codexHome = join(sb.root, `codex-${item.plugin}`);
  const pluginRoot = join(ROOT, "plugins", item.plugin);
  // A ported hook is a committed bundle (#269); a native hook runs `toolu`; the rest are bash.
  const entry = script.replace(/\.sh$/, "");
  const binary = builtTooluBinary();
  const argv = exists(bundlePath(pluginRoot, entry))
    ? entryArgv(item.plugin, entry, pluginRoot)
    : binary !== undefined && !exists(join(pluginRoot, "hooks", script))
      ? [binary, item.plugin, "hook", entry, "--event", "SessionStart", "--plugin-root", pluginRoot]
      : ["bash", join(pluginRoot, "hooks", script)];
  const res = await run(argv, {
    cwd: sb.project,
    env: {
      CLAUDE_CONFIG_DIR: undefined,
      HOME: sb.home,
      CODEX_HOME: codexHome,
      PLUGIN_ROOT: pluginRoot,
    },
    stdin: "{}",
  });
  return { exitCode: res.exitCode, codexHome };
}

function exists(path: string): boolean {
  return lstatSync(path, { throwIfNoEntry: false }) !== undefined;
}

test.concurrent("Codex SessionStart wrapper publishers use CODEX_HOME", async () => {
  using sb = createSandbox();
  await Promise.all(
    SESSION_START.map(async (item) => {
      const { exitCode, codexHome } = await codexHook(sb, item, "session-start.sh");
      expect({ plugin: item.plugin, exitCode }).toEqual({ plugin: item.plugin, exitCode: 0 });
      expect(
        lstatSync(join(codexHome, item.published), { throwIfNoEntry: false })?.isSymbolicLink(),
      ).toBe(true);
      expect(exists(join(sb.home, ".claude", item.published))).toBe(false);
    }),
  );
});

test.concurrent("Codex registry publishers use the shared CODEX_HOME registry", async () => {
  using sb = createSandbox();
  await Promise.all(
    REGISTRY.map(async (item) => {
      const { exitCode, codexHome } = await codexHook(sb, item, "register.sh");
      expect({ plugin: item.plugin, exitCode }).toEqual({ plugin: item.plugin, exitCode: 0 });
      expect(
        statSync(join(codexHome, "toolu", item.published), { throwIfNoEntry: false })?.isFile(),
      ).toBe(true);
      expect(exists(join(sb.home, ".claude/toolu", item.published))).toBe(false);
    }),
  );
});
