/**
 * Live startup scenarios (#342): the pinned host loads `@toolu/opencode` through a
 * local shim and toolu runs every selected plugin's startup. Readiness is read
 * from toolu's host-log diagnostic, enforcement from the tool states, and the
 * contributions from the project's data root on disk.
 */
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { z } from "zod";
import { listPluginManifests } from "../../../tools/toolu-opencode/src/inventory/scan.ts";
import { runHost, toolStates } from "./host-run.ts";
import type { Scripts } from "./provider.ts";
import { finalMessages, messagesText } from "./scenario.ts";
import {
  PROJECT_FILES,
  ROOT,
  TOUCH_SCRIPT,
  diagnostics,
  entrySession,
  installShim,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

/** Where the shimmed package bootstraps a project that sets no data-root override. */
export const DATA_ROOT = ".opencode/toolu/state";

const ALLOWED_SCRIPT: Scripts = {
  "startup.touch": [{ tool: "bash", args: { command: "touch allowed.txt", description: "x" } }],
};

const HookFile = z.looseObject({
  hooks: z.looseObject({
    SessionStart: z.array(z.looseObject({ hooks: z.array(z.unknown()) })),
  }),
});

function selection(names: readonly string[]): string {
  return JSON.stringify({ version: 1, enabled: names });
}

function shimmedSession(
  ctx: EntryContext,
  names: readonly string[],
  scripts: Scripts,
  repoRoot: string,
): ProbeSession {
  const files = { ...PROJECT_FILES, ".opencode/toolu/plugins.json": selection(names) };
  const s = entrySession(ctx, {
    config: () => ({ permission: { bash: "allow" } }),
    scripts,
    files,
  });
  installShim(s);
  s.env.TOOLU_REPO_ROOT = repoRoot;
  return s;
}

/** Registry modules on disk, as `<dir>/<file>`. */
export function modules(s: ProbeSession): string[] {
  return ["pre-tools.d", "post-tools.d"].flatMap((dir) => {
    const path = join(s.sb.project, DATA_ROOT, "toolu", dir);
    return existsSync(path) ? readdirSync(path).map((file) => `${dir}/${file}`) : [];
  });
}

/** The registry modules the retained catalog publishes and reports. */
export const MODULES = [
  "pre-tools.d/ast-grep@toolu__search-nudge.json",
  "post-tools.d/ast-grep@toolu__byte-savings.json",
  "post-tools.d/python-quality@toolu__python-quality.json",
  "post-tools.d/rust-quality@toolu__rust-quality.json",
  "post-tools.d/ts-quality@toolu__ts-quality.json",
];

/** Every registry file on disk: the modules plus the quality plugins' unreported bridges for Bun dispatch. */
export const REGISTRY_FILES = [
  ...MODULES,
  "post-tools.d/python-quality@toolu__python-quality.sh",
  "post-tools.d/rust-quality@toolu__rust-quality.sh",
  "post-tools.d/ts-quality@toolu__ts-quality.sh",
];

/** The helper symlinks the retained catalog publishes into the data root. */
export const HELPERS = ["jev/jev.sh", "statusline/statusline.sh", "toolu-review/write-state.sh"];

async function fullStartup(ctx: EntryContext): Promise<EntryResult> {
  const names = (listPluginManifests(join(ROOT, "plugins")) ?? []).map((p) => p.name);
  using s = shimmedSession(ctx, names, ALLOWED_SCRIPT, ROOT);
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
  const bash = toolStates(hostRun).find((state) => state.tool === "bash");
  const observed = {
    plugins: names.length,
    ready: diagnostics(
      hostRun.stderr,
      `toolu: ready (${names.length} plugins, ${MODULES.length + HELPERS.length} startup artifacts)`,
    ),
    notReady: diagnostics(hostRun.stderr, "toolu: not ready"),
    modules: modules(s).toSorted().join(",") === REGISTRY_FILES.toSorted().join(","),
    helpers: HELPERS.filter((path) => existsSync(join(s.sb.project, DATA_ROOT, path))).length,
    bashRan: bash?.status === "completed" && s.exists("allowed.txt"),
  };
  const pass =
    observed.plugins === 12 &&
    observed.ready === 1 &&
    observed.notReady === 0 &&
    observed.modules &&
    observed.helpers === HELPERS.length &&
    observed.bashRan;
  return { pass, observed };
}

/** A register command typed by hand instead of generated: the bootstrap refuses to run it. */
const HAND_WRITTEN = "toolu python-quality hook register";

/** Replace the first SessionStart command of the `hooks.json` at `path` with {@link HAND_WRITTEN}. */
function handWrittenRegister(path: string): void {
  const HooksFile = z.looseObject({
    hooks: z.looseObject({
      SessionStart: z.array(
        z.looseObject({ hooks: z.array(z.looseObject({ command: z.string() })) }),
      ),
    }),
  });
  const file = HooksFile.parse(JSON.parse(readFileSync(path, "utf8")));
  const first = file.hooks.SessionStart[0]?.hooks[0];
  if (first === undefined) throw new Error(`${path}: no SessionStart command`);
  first.command = HAND_WRITTEN;
  writeFileSync(path, JSON.stringify(file));
}

async function startupFailure(ctx: EntryContext): Promise<EntryResult> {
  const catalog = mkdtempSync(join(tmpdir(), "toolu-startup-catalog-"));
  try {
    cpSync(join(ROOT, "plugins"), join(catalog, "plugins"), { recursive: true });
    handWrittenRegister(join(catalog, "plugins/python-quality/hooks/hooks.json"));
    using s = shimmedSession(ctx, ["toolu", "python-quality"], TOUCH_SCRIPT, catalog);
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:entry.touch"]);
    const bash = toolStates(hostRun).find((state) => state.tool === "bash");
    const observed = {
      ready: diagnostics(hostRun.stderr, "toolu: ready"),
      // The log quotes the message, so the command's own quotes are escaped there.
      notReady: diagnostics(
        hostRun.stderr,
        "toolu: not ready: bootstrap: python-quality: unsupported SessionStart command ",
      ),
      bashDenied: bash?.status === "error" && (bash.error ?? "").startsWith("toolu: not ready"),
      fileAbsent: !s.exists("denied.txt"),
    };
    const pass =
      observed.ready === 0 && observed.notReady === 1 && observed.bashDenied && observed.fileAbsent;
    return { pass, observed };
  } finally {
    rmSync(catalog, { recursive: true, force: true });
  }
}

async function startupDisable(ctx: EntryContext): Promise<EntryResult> {
  using s = shimmedSession(ctx, ["toolu", "ast-grep"], ALLOWED_SCRIPT, ROOT);
  const first = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
  const before = modules(s).filter((module) => module.includes("ast-grep@toolu__")).length;
  s.sb.write(".opencode/toolu/plugins.json", selection(["toolu"]));
  const second = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
  const observed = {
    firstReady: diagnostics(first.stderr, "toolu: ready (2 plugins"),
    secondReady: diagnostics(second.stderr, "toolu: ready (1 plugins"),
    astGrepBefore: before,
    astGrepAfter: modules(s).filter((module) => module.includes("ast-grep@toolu__")).length,
  };
  const pass =
    observed.firstReady === 1 &&
    observed.secondReady === 1 &&
    observed.astGrepBefore === 2 &&
    observed.astGrepAfter === 0;
  return { pass, observed };
}

/** Switch one copied hook to a Rust-generated native launcher, retaining the rest of its routing. */
function useNativeLauncher(catalog: string, plugin: string): void {
  const path = join(catalog, `plugins/${plugin}/hooks/hooks.json`);
  const fixture = join(
    ROOT,
    `tooling/fixtures/native-launcher/${plugin === "toolu" ? "" : `${plugin}-`}session-start.json`,
  );
  const hooks = HookFile.parse(JSON.parse(readFileSync(path, "utf8")));
  const first = hooks.hooks.SessionStart[0]?.hooks;
  if (first === undefined || first.length === 0) throw new Error(`${plugin}: no SessionStart hook`);
  const nativeHook: unknown = JSON.parse(readFileSync(fixture, "utf8"));
  first[0] = nativeHook;
  writeFileSync(path, `${JSON.stringify(hooks, null, 2)}\n`);
}

async function withNativeCatalog(
  plugin: string,
  run: (catalog: string) => Promise<EntryResult>,
): Promise<EntryResult> {
  const catalog = mkdtempSync(join(tmpdir(), "toolu-native-catalog-"));
  try {
    cpSync(join(ROOT, "plugins"), join(catalog, "plugins"), { recursive: true });
    useNativeLauncher(catalog, plugin);
    return await run(catalog);
  } finally {
    rmSync(catalog, { recursive: true, force: true });
  }
}

async function nativeContext(ctx: EntryContext): Promise<EntryResult> {
  return withNativeCatalog("toolu", async (catalog) => {
    const executable = join(catalog, "native-toolu");
    writeFileSync(
      executable,
      '#!/bin/sh\nif [ "$1" = "--hook-protocol" ]; then printf "1\\n"; exit 0; fi\nif [ "$1" = "hook" ] && [ "$2" = "session-start" ]; then printf \'%s\\n\' \'{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"native launcher context reached OpenCode"}}\'; exit 0; fi\nif [ "$1" = "hook" ]; then exit 0; fi\nexit 3\n',
    );
    chmodSync(executable, 0o755);
    using s = shimmedSession(ctx, ["toolu"], ALLOWED_SCRIPT, catalog);
    s.env.TOOLU_BIN = executable;
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
    const context = messagesText(s, "system");
    const observed = {
      hostExit: hostRun.exitCode,
      ready: diagnostics(hostRun.stderr, "toolu: ready"),
      delivered: context.includes("native launcher context reached OpenCode"),
    };
    return {
      pass: observed.hostExit === 0 && observed.ready === 1 && observed.delivered,
      observed,
    };
  });
}

async function nativeMissingBinary(ctx: EntryContext): Promise<EntryResult> {
  return withNativeCatalog("toolu", async (catalog) => {
    using s = shimmedSession(ctx, ["toolu"], ALLOWED_SCRIPT, catalog);
    s.env.TOOLU_BIN = s.outside("missing-toolu");
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
    const context = messagesText(s, "system");
    const installNotice = "toolu plugin: no usable native toolu";
    const observed = {
      hostExit: hostRun.exitCode,
      ready: diagnostics(hostRun.stderr, "toolu: ready"),
      installer: context.includes("curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash"),
      homebrew: context.includes("brew install falconiere/tap/toolu"),
      restart: context.includes("Then restart the session"),
      modelCopies: finalMessages(s, "system").join("\n").split(installNotice).length - 1,
      hostLogCopies: hostRun.stderr.split(installNotice).length - 1,
    };
    return {
      pass:
        observed.hostExit === 0 &&
        observed.ready === 1 &&
        observed.installer &&
        observed.homebrew &&
        observed.restart &&
        observed.modelCopies === 1 &&
        // The core plugin now invokes multiple native hooks in this session.
        observed.hostLogCopies === 3,
      observed,
    };
  });
}

async function nativeStatuslineMissingBinary(ctx: EntryContext): Promise<EntryResult> {
  return withNativeCatalog("statusline", async (catalog) => {
    using s = shimmedSession(ctx, ["statusline"], ALLOWED_SCRIPT, catalog);
    s.env.TOOLU_BIN = "";
    // An empty TOOLU_BIN still searches PATH. Drop any directory that holds toolu
    // so this case exercises the install advisory without a native binary.
    s.env.PATH = (s.env.PATH ?? process.env.PATH ?? "")
      .split(":")
      .filter((dir) => dir.length > 0 && !existsSync(join(dir, "toolu")))
      .join(":");
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:startup.touch"]);
    const helper = join(s.sb.project, DATA_ROOT, "statusline/statusline.sh");
    const context = messagesText(s, "system");
    const observed = {
      hostExit: hostRun.exitCode,
      ready: diagnostics(hostRun.stderr, "toolu: ready"),
      installNotice: context.includes("statusline plugin: no usable native toolu"),
      installer: context.includes("curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash"),
      helperAbsent: !existsSync(helper),
    };
    return {
      pass:
        observed.hostExit === 0 &&
        observed.ready === 1 &&
        observed.installNotice &&
        observed.installer &&
        observed.helperAbsent,
      observed,
    };
  });
}

export const STARTUP_SCENARIOS: EntryScenario[] = [
  {
    id: "entry.full-startup",
    claim:
      "With all 12 plugins enabled, every startup runs and its modules and helpers are in place",
    run: fullStartup,
  },
  {
    id: "entry.startup-failure",
    claim:
      "A selected plugin whose registration cannot complete leaves toolu not ready and refuses tools",
    run: startupFailure,
  },
  {
    id: "entry.startup-disable",
    claim: "Disabling a plugin removes its registry modules at the next startup",
    run: startupDisable,
  },
  {
    id: "entry.native-context",
    claim: "A generated native SessionStart launcher delivers its context through the pinned host",
    run: nativeContext,
  },
  {
    id: "entry.native-missing-binary",
    claim:
      "A missing native binary leaves OpenCode ready and sends both install commands to the model",
    run: nativeMissingBinary,
  },
  {
    id: "entry.native-statusline-missing-binary",
    claim: "Without native toolu, statusline advises installation and publishes no stale helper",
    run: nativeStatuslineMissingBinary,
  },
];
