/** Jira epics through the toolu jira CLI (published as `jira.sh`, a Bun bundle run by path), which owns auth
 * (env vars or the jira CLI login). Children are `parent = EPIC` (Cloud and
 * team-managed) or `"Epic Link" = EPIC` (Server/DC); blockers are issue links
 * whose wording from the child's side is "is blocked by" or "depends on". */

import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { CommandError, helperCandidates, run } from "../common.ts";
import { defaultPolicy, withRetry } from "../ratelimit.ts";
import { excerpt, repoFor, type EpicInfo, type TrackedIssue, type Tracker } from "./types.ts";

export const JIRA_KEY = /^[A-Z][A-Z0-9_]+-\d+$/;
const BROWSE = /\/browse\/([A-Z][A-Z0-9_]+-\d+)/i;
const BLOCKING = /\bblocked by\b|\bdepends on\b/i;
const FIELDS = ["summary", "status", "issuelinks", "description", "labels"];

type Status = { statusCategory?: { key?: string } };
type LinkedIssue = { key: string; fields?: { status?: Status } };
type Link = {
  type: { inward: string; outward: string };
  inwardIssue?: LinkedIssue;
  outwardIssue?: LinkedIssue;
};
type JiraIssue = {
  key: string;
  fields: {
    summary?: string;
    status?: Status;
    issuelinks?: Link[];
    description?: unknown;
    labels?: string[];
  };
};

export function parseJiraRef(ref: string): string | null {
  const t = ref.trim().replace(/^jira:/i, "");
  const url = BROWSE.exec(t);
  if (url?.[1]) return url[1].toUpperCase();
  return JIRA_KEY.test(t.toUpperCase()) && /^[a-z]/i.test(t) ? t.toUpperCase() : null;
}

export function jiraScript(env: NodeJS.ProcessEnv = process.env): string | null {
  const candidates = [env.EPIC_JIRA_SH, ...helperCandidates("jira/jira.sh", env)];
  return candidates.find((p): p is string => !!p && existsSync(p)) ?? null;
}

export function jiraConfigured(env: NodeJS.ProcessEnv = process.env): boolean {
  const home = env.HOME ?? homedir();
  return !!env.JIRA_BASE_URL || existsSync(join(home, ".config", ".jira", ".config.yml"));
}

function closedState(s: Status | undefined): "open" | "closed" {
  return s?.statusCategory?.key === "done" ? "closed" : "open";
}

/** Blockers of one issue from its links, keyed by issue key. */
export function jiraBlockers(links: Link[] | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  for (const l of links ?? []) {
    const other = l.inwardIssue ?? l.outwardIssue;
    const wording = l.inwardIssue ? l.type.inward : l.type.outward;
    if (other && BLOCKING.test(wording)) out[other.key] = closedState(other.fields?.status);
  }
  return out;
}

/** Plain text from an Atlassian Document Format node (v3) or a v2 string. */
export function adfText(node: unknown): string {
  if (typeof node === "string") return node;
  if (!node || typeof node !== "object") return "";
  const n = node as { text?: string; content?: unknown[]; type?: string };
  const inner = (n.content ?? []).map(adfText).join(n.type === "doc" ? "\n" : "");
  return (n.text ?? "") + inner + (n.type === "paragraph" ? "\n" : "");
}

export class JiraTracker implements Tracker {
  readonly kind = "jira";
  readonly epicRef: string;
  private readonly script: string;
  private readonly apiVersion = process.env.JIRA_API_VERSION === "2" ? "2" : "3";

  constructor(
    ref: string,
    private readonly defaultRepo: string,
  ) {
    const key = parseJiraRef(ref);
    if (!key) throw new Error(`not a Jira issue key or browse URL: ${ref}`);
    this.epicRef = key;
    const script = jiraScript();
    if (!script) {
      throw new Error("jira.sh not found; install the toolu jira plugin or set EPIC_JIRA_SH");
    }
    this.script = script;
  }

  private async call(args: string[], write = false): Promise<unknown> {
    const out = await withRetry(() => run([this.script, ...args]), {
      ...defaultPolicy(),
      retryTransient: !write,
    });
    return out.trim() ? JSON.parse(out) : null;
  }

  private api(path: string): string {
    return `/rest/api/${this.apiVersion}/${path}`;
  }

  private async search(jql: string): Promise<JiraIssue[]> {
    const out: JiraIssue[] = [];
    let token: string | undefined;
    for (let page = 0; page < 20; page++) {
      if (this.apiVersion === "2") {
        const body = { jql, fields: FIELDS, maxResults: 100, startAt: out.length };
        const res = (await this.call([
          "raw",
          "POST",
          this.api("search"),
          JSON.stringify(body),
        ])) as {
          issues: JiraIssue[];
          total: number;
        };
        out.push(...res.issues);
        if (out.length >= res.total || res.issues.length === 0) break;
      } else {
        const body = {
          jql,
          fields: FIELDS,
          maxResults: 100,
          ...(token ? { nextPageToken: token } : {}),
        };
        const res = (await this.call([
          "raw",
          "POST",
          this.api("search/jql"),
          JSON.stringify(body),
        ])) as {
          issues: JiraIssue[];
          nextPageToken?: string;
        };
        out.push(...res.issues);
        token = res.nextPageToken;
        if (!token) break;
      }
    }
    return out;
  }

  async epic(): Promise<EpicInfo> {
    const e = (await this.call([
      "raw",
      "GET",
      this.api(`issue/${this.epicRef}?fields=summary,status`),
    ])) as JiraIssue;
    const base = (process.env.JIRA_BASE_URL ?? "").replace(/\/$/, "");
    return {
      ref: this.epicRef,
      title: e.fields.summary ?? this.epicRef,
      state: closedState(e.fields.status),
      url: base ? `${base}/browse/${this.epicRef}` : this.epicRef,
    };
  }

  async children(): Promise<TrackedIssue[]> {
    let items = await this.search(`parent = ${this.epicRef} ORDER BY key`);
    if (items.length === 0) {
      try {
        items = await this.search(`"Epic Link" = ${this.epicRef} ORDER BY key`);
      } catch (err) {
        // Cloud rejects the retired Epic Link field; parent was the answer.
        if (!(err instanceof CommandError)) throw err;
      }
    }
    const base = (process.env.JIRA_BASE_URL ?? "").replace(/\/$/, "");
    return items.map((i) => {
      const labels = i.fields.labels ?? [];
      const body = adfText(i.fields.description);
      return {
        ref: i.key,
        title: i.fields.summary ?? i.key,
        state: closedState(i.fields.status),
        url: base ? `${base}/browse/${i.key}` : i.key,
        repo: repoFor(labels, body, this.defaultRepo),
        number: null,
        blockers: jiraBlockers(i.fields.issuelinks),
        prs: [],
        deps_source: "links",
        labels,
        excerpt: excerpt(body),
      };
    });
  }

  private async done(key: string): Promise<boolean> {
    const e = (await this.call([
      "raw",
      "GET",
      this.api(`issue/${key}?fields=status`),
    ])) as JiraIssue;
    return closedState(e.fields.status) === "closed";
  }

  /** Move to the first transition that lands in the Done status category. */
  private async transitionDone(key: string): Promise<boolean> {
    const res = (await this.call(["raw", "GET", this.api(`issue/${key}/transitions`)])) as {
      transitions: { id: string; to?: Status }[];
    };
    const t = res.transitions.find((x) => x.to?.statusCategory?.key === "done");
    if (!t) return false;
    await this.call(["issue", "transition", key, t.id], true);
    return true;
  }

  async closeIssue(key: string, note: string): Promise<string> {
    if (await this.done(key)) return "already-done";
    await this.call(["issue", "comment", key, note], true);
    return (await this.transitionDone(key)) ? "transitioned-done" : "no-done-transition";
  }

  tickEpic(): Promise<boolean> {
    // Jira tracks children through the parent field, not a body checklist.
    return Promise.resolve(false);
  }

  async closeEpic(summary: string): Promise<string> {
    await this.call(["issue", "comment", this.epicRef, summary], true);
    return (await this.transitionDone(this.epicRef)) ? "transitioned-done" : "no-done-transition";
  }

  stateSlug(): string {
    return `jira-${this.epicRef}`.toLowerCase();
  }
}
