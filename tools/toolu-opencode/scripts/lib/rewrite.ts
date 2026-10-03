/** Body rewrites and reference cataloging (#206). */
import { posix } from "node:path";
import { pluginRootVar } from "../../src/host/runtime-env.ts";
import { CLAUDE_PLUGIN_ROOT, TOOLU_OPENCODE_ROOT } from "./constants.ts";

export type RewriteNotes = {
  claudePluginRootRewrites: number;
  explicitReferenceRewrites: number;
  sourcePathRewrites: number;
  claudeConfigRewrites: number;
  dotClaudeRefs: string[];
};

export type SurfaceReferences = {
  invocations: ReadonlyMap<string, string>;
  paths: ReadonlyMap<string, string>;
};

/** Match remaining host path tokens after CLAUDE_PLUGIN_ROOT rewrite. */
const DOT_CLAUDE_PATH = /(?:^|[\s"'`(/=])\.claude(?:\/|["'`)\s]|$)/;

/** Whose surface is being rewritten: its plugin, and its id when it is a skill. */
export type RewriteOwner = { plugin: string; skillId?: string };

const OPENCODE_CONFIG = "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}";

/** Jev's Codex assignment and the commented other-host line after it, in either source layout. */
const JEV_HOST_PAIR =
  /# Codex[^\n]*\nJEV="\$\{TOOLU_CONFIG_DIR:-\$\{CODEX_HOME:-\$HOME\/\.codex\}\}\/jev\/jev\.sh"\n(?:# Claude Code[^\n]*\n)?# JEV="[^"\n]*"/g;

/** Jev on OpenCode (#350): the one wrapper `shell.env` resolves, run by a Bun that ignores `.env`. */
export function opencodeJev(text: string): string {
  return text
    .replace(JEV_HOST_PAIR, `# OpenCode\nJEV="${OPENCODE_CONFIG}/jev/jev.sh"`)
    .replaceAll('"$JEV_BUN" "$JEV"', '"$JEV_BUN" --no-env-file "$JEV"');
}

/** context7's Codex and Claude Code command lines, after the config-root rewrite. */
const CONTEXT7_HOST_PAIR =
  /# Codex\n"\$\{TOOLU_CONFIG_DIR:-\$\{CODEX_HOME:-\$HOME\/\.codex\}\}\/context7\/search\.sh" <command> \[options\]\n# Claude Code\n"[^"\n]*\/context7\/search\.sh" <command> \[options\]/;
const CONTEXT7_CHOOSE =
  /Choose the line for the active host\. Ordinary shell calls do not inherit\nplugin lifecycle variables, so never collapse these into one ambiguous\nfallback\. Use the published path; plugin-root variables are lifecycle-only\./;

/** context7 on OpenCode (#348): one command, run by `shell.env`'s Bun, which ignores `.env`. */
export function opencodeContext7(text: string): string {
  return text
    .replace(
      CONTEXT7_HOST_PAIR,
      `# OpenCode\n"$TOOLU_BUN" --no-env-file "${OPENCODE_CONFIG}/context7/search.sh" <command> [options]`,
    )
    .replace(
      CONTEXT7_CHOOSE,
      "`shell.env` sets `TOOLU_BUN` and `TOOLU_CONFIG_DIR` in every bash call, so this\nruns with `bun` off `PATH` and never loads a project `.env`. A file of your own\nat that path runs directly instead. Plugin-root variables are lifecycle-only.",
    );
}

/** jira's Codex and Claude Code commands, after the config-root rewrite. */
const JIRA_HOST_PAIR =
  /# Codex\nTOOLU_HOST_OVERRIDE=codex \\\n {2}"\$\{TOOLU_CONFIG_DIR:-\$\{CODEX_HOME:-\$HOME\/\.codex\}\}\/jira\/jira\.sh" (\[--api-version N\] \[--lean\] <family> <action> \[options\])\n# Claude Code\nTOOLU_HOST_OVERRIDE=claude \\\n {2}"[^"\n]*\/jira\/jira\.sh" \1/;
const JIRA_CHOOSE =
  /Choose the complete command for the active host, including its override; every\n`jira\.sh` shorthand below means that chosen prefix\. The override propagates to\nnested plan checks and their state paths\. Ordinary shell calls do not inherit\nplugin lifecycle variables, so never collapse these into one ambiguous\nfallback\. Use the published path; plugin-root variables are lifecycle-only\./;

/** jira on OpenCode (#351): one command under `shell.env`'s Bun, and the `.opencode` state paths. */
export function opencodeJira(text: string): string {
  return text
    .replace(JIRA_HOST_PAIR, `# OpenCode\n"$TOOLU_BUN" --no-env-file "${OPENCODE_CONFIG}/jira/jira.sh" $1`)
    .replace(
      JIRA_CHOOSE,
      "`shell.env` sets `TOOLU_BUN`, `TOOLU_CONFIG_DIR` and `TOOLU_HOST_OVERRIDE=opencode`\nin every bash call, so this runs with `bun` off `PATH` and never loads a project\n`.env`; every `jira.sh` shorthand below means this command. Plan checks call\n`\"$JIRA\"` with `.env` loading off too. A file of your own at that path runs\ndirectly instead. Plugin-root variables are lifecycle-only.",
    )
    .replace(
      "`.claude/tmp/jira/plans/<KEY>.md` or `.codex/tmp/jira/plans/<KEY>.md`.",
      "`.opencode/tmp/jira/plans/<KEY>.md`.",
    )
    .replace(
      "below the active host's `<repo>/.claude/tmp/plan-ledger/`\nor `<repo>/.codex/tmp/plan-ledger/`.",
      "below `<repo>/.opencode/tmp/plan-ledger/`.",
    );
}

export function rewriteBody(
  body: string,
  references: SurfaceReferences,
  owner: RewriteOwner,
): { body: string; notes: RewriteNotes } {
  const { skillId } = owner;
  // Each plugin's own root, as `${CLAUDE_PLUGIN_ROOT}` is on Claude: shell.env sets one per enabled plugin.
  const parts = body.split(CLAUDE_PLUGIN_ROOT);
  const claudePluginRootRewrites = parts.length - 1;
  let rewritten = parts.join(`\${${pluginRootVar(owner.plugin)}}`);
  const claudeConfig = "${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}";
  const configParts = rewritten.split(claudeConfig);
  const claudeConfigRewrites = configParts.length - 1;
  rewritten = configParts.join(OPENCODE_CONFIG);
  let sourcePathRewrites = 0;
  for (const [source, destination] of [...references.paths].sort(
    (a, b) => b[0].length - a[0].length,
  )) {
    const pathParts = rewritten.split(source);
    sourcePathRewrites += pathParts.length - 1;
    rewritten = pathParts.join(destination);
  }
  if (skillId) {
    if (skillId === "agent-browser-agent-browser") {
      const cliStart = rewritten.indexOf("```bash\n# Codex\n");
      const wrapperStart = rewritten.indexOf("The wrapper bakes in token-lean defaults", cliStart);
      if (cliStart < 0 || wrapperStart < 0) {
        throw new Error("agent-browser skill: cannot find host-specific CLI instructions");
      }
      rewritten = `${rewritten.slice(0, cliStart)}\`\`\`bash
# OpenCode
: "\${TOOLU_CONFIG_DIR:?toolu OpenCode startup required}"
"\${TOOLU_CONFIG_DIR}/agent-browser/agent-browser.sh" <command> [args]
\`\`\`

OpenCode provides \`TOOLU_CONFIG_DIR\` for the current project in every bash call.
Use the helper path above; its SessionStart entry refreshes the symlink and
places the exact path in startup instructions.

${rewritten.slice(wrapperStart)}`;
      rewritten = rewritten.replace(
        "use the `context7` skill.",
        "use the native `context7-context7` skill when enabled; otherwise read official docs with OpenCode's `webfetch`.",
      );
      rewritten = rewritten.replace(
        "use `exa-search`\n  (or the active host's native web fetch).",
        "use the native `exa-search-exa-search` skill when enabled, or OpenCode's `webfetch`.",
      );
    }
    if (skillId === "exa-search-exa-search") {
      const cliStart = rewritten.indexOf("```bash\n# Codex\n");
      const fallbackStart = rewritten.indexOf("Repo-checkout fallback", cliStart);
      if (cliStart < 0 || fallbackStart < 0) {
        throw new Error("exa-search skill: cannot find host-specific CLI instructions");
      }
      rewritten = `${rewritten.slice(0, cliStart)}\`\`\`bash
# OpenCode
"\${TOOLU_CONFIG_DIR}/exa-search/search.sh" <command> [options]
\`\`\`

OpenCode sets \`TOOLU_CONFIG_DIR\` to this project's helper root in every bash call.
The exa-search SessionStart entry publishes the helper and gives its exact path.
If \`EXA_API_KEY\` is unset, use OpenCode's \`websearch\` when available or
\`webfetch\` for a known URL. Do not call the Exa helper until the key is set,
and never print the key.

${rewritten.slice(fallbackStart)}`;
      rewritten = rewritten.replace(
        /^search\.sh(?= )/gm,
        '"${TOOLU_CONFIG_DIR}/exa-search/search.sh"',
      );
    }
    if (skillId === "jev-jev") rewritten = opencodeJev(rewritten);
    if (skillId === "context7-context7") rewritten = opencodeContext7(rewritten);
    if (skillId === "jira-jira") rewritten = opencodeJira(rewritten);
    if (skillId === "statusline-status") {
      rewritten = rewritten.replace("in Codex.", "in OpenCode.");
      rewritten = rewritten.replace(
        "TOOLU_HOST_OVERRIDE=codex bun ../../hooks/dist/status.js",
        "TOOLU_HOST_OVERRIDE=opencode bun ../../../plugins/statusline/hooks/dist/status.js",
      );
    }
    const modelRouting = `${TOOLU_OPENCODE_ROOT}/generated/skills/toolu-orchestrator/references/model-routing.md`;
    const relativeRouting = posix.relative(
      `skills/${skillId}`,
      "skills/toolu-orchestrator/references/model-routing.md",
    );
    rewritten = rewritten.replaceAll(`\`${modelRouting}\``, `\`${relativeRouting}\``);
  }
  let explicitReferenceRewrites = 0;
  for (const [reference, id] of [...references.invocations].sort(
    (a, b) => b[0].length - a[0].length,
  )) {
    const escaped = reference.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const paired = new RegExp(`\x60([$/])${escaped}\x60\\s+or\\s+\x60([$/])${escaped}\x60`, "g");
    rewritten = rewritten.replace(paired, (match, first: string, second: string) => {
      if (first === second) return match;
      explicitReferenceRewrites += 2;
      return `\x60skill({ name: "${id}" })\x60`;
    });
    const token = new RegExp(`([$/])${escaped}(?![A-Za-z0-9-])`, "g");
    rewritten = rewritten.replace(token, () => {
      explicitReferenceRewrites += 1;
      return `skill({ name: "${id}" })`;
    });
    if (reference === "brainstorm:brainstorm") {
      rewritten = rewritten.replaceAll(`\`${reference}\``, `\`${id}\``);
    }
  }

  const dotClaudeRefs: string[] = [];
  for (const line of rewritten.split("\n")) {
    if (DOT_CLAUDE_PATH.test(line)) {
      dotClaudeRefs.push(line.trim());
    }
  }

  return {
    body: rewritten,
    notes: {
      claudePluginRootRewrites,
      explicitReferenceRewrites,
      sourcePathRewrites,
      claudeConfigRewrites,
      dotClaudeRefs: [...new Set(dotClaudeRefs)].sort(),
    },
  };
}
