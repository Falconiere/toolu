/** Worker hosts: how each agent CLI is started by `herdr agent start`, which
 * flags skip its approval prompts, how model and effort are passed, how a
 * relaunch resumes the previous conversation, and how skills are invoked.
 *
 * Every arg must be shell-safe (no quotes, brackets, or spaces): herdr types
 * the command into the pane's interactive shell. */

export const HOST_KINDS = ["claude", "codex", "cursor", "opencode"] as const;
export type HostKind = (typeof HOST_KINDS)[number];

const ALIASES: Record<string, HostKind> = {
  claude: "claude",
  "claude-code": "claude",
  codex: "codex",
  cursor: "cursor",
  "cursor-agent": "cursor",
  opencode: "opencode",
};

/** The host a name or alias refers to, or null when it names no host. */
export function parseHostKind(name: string): HostKind | null {
  return ALIASES[name.trim().toLowerCase()] ?? null;
}

export function hostKind(name: string): HostKind {
  const kind = parseHostKind(name);
  if (!kind) throw new Error(`unknown host ${name}; use one of ${HOST_KINDS.join(", ")}`);
  return kind;
}

type HostSpec = {
  /** Unattended: approvals and sandbox prompts off. */
  bypass: string[];
  /** Attended fallback (`--safe`). */
  safe: (permissionMode: string) => string[];
  model: (model: string) => string[];
  effort: (effort: string) => string[];
  name: (key: string) => string[];
  /** Args that continue the most recent session in the worktree. `lead`
   * args go first (codex uses a subcommand). */
  resume: { lead: string[]; flags: string[] };
  /** How the brief tells this host to run a toolu skill. */
  skill: (plugin: string, skill: string) => string;
};

/** The generated OpenCode skill name for each `plugin:skill` a brief names
 * (tools/toolu-opencode/generated/skills/<id>/SKILL.md). */
export const OPENCODE_SKILL_IDS: Readonly<Record<string, string>> = {
  "delivery-flow:delivery-flow": "delivery-flow-delivery-flow",
  "pr-babysit:babysit": "pr-babysit-babysit-73c340c6",
  "toolu:debug": "toolu-debug",
};

/** The TUI takes provider/model and has no variant flag. */
export function opencodeModelArgs(model: string): string[] {
  if (!/^[^/\s]+\/\S+$/.test(model)) {
    throw new Error(`OpenCode model must be provider/model, got ${model}`);
  }
  return ["--model", model];
}

/** Why `opencode --version` output is not a release toolu's OpenCode plugin
 * targets (opencode-ai 1.x), or null when it is. */
export function opencodeVersionProblem(versionOutput: string): string | null {
  const version = /\d+\.\d+\.\d+/.exec(versionOutput)?.[0];
  if (version?.startsWith("1.")) return null;
  const seen = version === undefined ? "printed no version" : `reports "${version}"`;
  return `opencode --version ${seen}; toolu's OpenCode plugin targets opencode-ai 1.x. Put a 1.x opencode first on PATH, or route this issue to another host.`;
}

const HOSTS: Record<HostKind, HostSpec> = {
  claude: {
    bypass: ["--dangerously-skip-permissions"],
    safe: (mode) => ["--permission-mode", mode],
    model: (m) => ["--model", m],
    effort: (e) => ["--effort", e],
    name: (key) => ["-n", key],
    resume: { lead: [], flags: ["--continue"] },
    skill: (p, s) => `\`/${p}:${s}\``,
  },
  codex: {
    bypass: ["--dangerously-bypass-approvals-and-sandbox"],
    safe: () => ["--ask-for-approval", "on-request", "--sandbox", "workspace-write"],
    model: (m) => ["--model", m],
    // Unquoted TOML fails to parse, so codex takes the raw string literally.
    effort: (e) => ["-c", `model_reasoning_effort=${e}`],
    name: () => [],
    resume: { lead: ["resume", "--last"], flags: [] },
    skill: (p, s) => `\`$${p}:${s}\``,
  },
  cursor: {
    bypass: ["--yolo", "--trust", "--approve-mcps"],
    safe: () => ["--trust"],
    model: (m) => ["--model", m],
    // Cursor encodes effort in the model id (gpt-5.6-sol-high); the routing
    // table picks the right id, so there is no separate flag.
    effort: () => [],
    name: () => [],
    resume: { lead: [], flags: ["--continue"] },
    skill: (p, s) => `the \`${p}:${s}\` skill`,
  },
  opencode: {
    bypass: ["--auto"],
    safe: () => [],
    model: opencodeModelArgs,
    effort: () => [],
    name: () => [],
    resume: { lead: [], flags: ["--continue"] },
    skill: (p, s) => {
      const id = OPENCODE_SKILL_IDS[`${p}:${s}`];
      if (id === undefined) throw new Error(`no OpenCode skill id for ${p}:${s}`);
      return `\`skill({ name: "${id}" })\``;
    },
  },
};

export type AgentArgOpts = {
  key: string;
  model?: string | undefined;
  effort?: string | undefined;
  bypass: boolean;
  permissionMode: string;
  resume: boolean;
  sessionId?: string;
};

const SHELL_SAFE = /^[A-Za-z0-9_./:=,@%+#-]+$/;

export function agentArgs(kind: HostKind, o: AgentArgOpts): string[] {
  const h = HOSTS[kind];
  const exactResume =
    o.resume && o.sessionId
      ? kind === "codex"
        ? ["resume", o.sessionId]
        : [kind === "opencode" ? "--session" : "--resume", o.sessionId]
      : null;
  const args = [
    ...(kind === "opencode" ? ["--standalone"] : []),
    ...(exactResume ?? (o.resume ? h.resume.lead : [])),
    ...(kind === "codex" ? ["--no-daemon"] : []),
    ...(o.bypass ? h.bypass : h.safe(o.permissionMode)),
    ...h.name(o.key),
    ...(o.model ? h.model(o.model) : []),
    ...(o.effort ? h.effort(o.effort) : []),
    ...(!exactResume && o.resume ? h.resume.flags : []),
  ];
  const bad = args.find((a) => !SHELL_SAFE.test(a));
  if (bad !== undefined) throw new Error(`unsafe ${kind} arg for the pane shell: ${bad}`);
  return args;
}

export function skillRef(kind: HostKind, plugin: string, skill: string): string {
  return HOSTS[kind].skill(plugin, skill);
}

/** Usage-limit or rate-limit messages the agent CLIs print when the provider
 * throttles them. Matched only against the last lines of an idle pane. */
export const HOST_LIMIT =
  /usage limit|rate[- ]limit(ed)?\b|hit your (usage )?limit|quota (exceeded|reached)|too many requests|\b429\b|limit will reset|try again (at|in) /i;
