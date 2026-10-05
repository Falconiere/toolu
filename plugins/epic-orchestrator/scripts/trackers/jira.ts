/** Jira epics through the built-in REST client. Children are `parent = EPIC` (Cloud and
 * team-managed) or `"Epic Link" = EPIC` (Server/DC); blockers are issue links
 * whose wording from the child's side is "is blocked by" or "depends on". */

import { CliExit } from "@toolu/core/cli";
import { excerpt, repoFor, type EpicInfo, type TrackedIssue, type Tracker } from "./types.ts";
import { JiraClient, type JiraLink, type JiraStatus } from "./jira-client.ts";

export const JIRA_KEY = /^[A-Z][A-Z0-9_]+-\d+$/;
const BROWSE = /\/browse\/([A-Z][A-Z0-9_]+-\d+)/i;
const BLOCKING = /\bblocked by\b|\bdepends on\b/i;

export function parseJiraRef(ref: string): string | null {
  const t = ref.trim().replace(/^jira:/i, "");
  const url = BROWSE.exec(t);
  if (url?.[1]) return url[1].toUpperCase();
  return JIRA_KEY.test(t.toUpperCase()) && /^[a-z]/i.test(t) ? t.toUpperCase() : null;
}

export function jiraConfigured(env: NodeJS.ProcessEnv = process.env): boolean {
  return !!env.JIRA_BASE_URL;
}

function closedState(s: JiraStatus | undefined): "open" | "closed" {
  return s?.statusCategory?.key === "done" ? "closed" : "open";
}

/** Blockers of one issue from its links, keyed by issue key. */
export function jiraBlockers(links: JiraLink[] | undefined): Record<string, string> {
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
  const text = "text" in node && typeof node.text === "string" ? node.text : "";
  const content = "content" in node && Array.isArray(node.content) ? node.content : [];
  const type = "type" in node && typeof node.type === "string" ? node.type : "";
  const inner = content.map(adfText).join(type === "doc" ? "\n" : "");
  return text + inner + (type === "paragraph" ? "\n" : "");
}

export class JiraTracker implements Tracker {
  readonly kind = "jira";
  readonly epicRef: string;
  private readonly client: JiraClient;

  constructor(
    ref: string,
    private readonly defaultRepo: string,
  ) {
    const key = parseJiraRef(ref);
    if (!key) throw new Error(`not a Jira issue key or browse URL: ${ref}`);
    this.epicRef = key;
    this.client = new JiraClient();
  }

  async epic(): Promise<EpicInfo> {
    const e = await this.client.getIssue(this.epicRef, ["summary", "status"]);
    return {
      ref: this.epicRef,
      title: e.fields.summary ?? this.epicRef,
      state: closedState(e.fields.status),
      url: `${this.client.base}/browse/${this.epicRef}`,
    };
  }

  async children(): Promise<TrackedIssue[]> {
    let items = await this.client.search(`parent = ${this.epicRef} ORDER BY key`);
    if (items.length === 0) {
      try {
        items = await this.client.search(`"Epic Link" = ${this.epicRef} ORDER BY key`);
      } catch (err) {
        // Cloud rejects the retired Epic Link field; parent was the answer.
        if (!(err instanceof CliExit) || err.code !== 22 || !err.message.includes("HTTP 400"))
          throw err;
      }
    }
    return items.map((i) => {
      const labels = i.fields.labels ?? [];
      const body = adfText(i.fields.description);
      return {
        ref: i.key,
        title: i.fields.summary ?? i.key,
        state: closedState(i.fields.status),
        url: `${this.client.base}/browse/${i.key}`,
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
    const e = await this.client.getIssue(key, ["status"]);
    return closedState(e.fields.status) === "closed";
  }

  /** Move to the first transition that lands in the Done status category. */
  private async transitionDone(key: string): Promise<boolean> {
    const transitions = await this.client.transitions(key);
    const t = transitions.find((x) => x.to?.statusCategory?.key === "done");
    if (!t) return false;
    await this.client.transition(key, t.name);
    return true;
  }

  async closeIssue(key: string, note: string): Promise<string> {
    if (await this.done(key)) return "already-done";
    await this.client.comment(key, note);
    return (await this.transitionDone(key)) ? "transitioned-done" : "no-done-transition";
  }

  tickEpic(): Promise<boolean> {
    // Jira tracks children through the parent field, not a body checklist.
    return Promise.resolve(false);
  }

  async closeEpic(summary: string): Promise<string> {
    await this.client.comment(this.epicRef, summary);
    return (await this.transitionDone(this.epicRef)) ? "transitioned-done" : "no-done-transition";
  }

  stateSlug(): string {
    return `jira-${this.epicRef}`.toLowerCase();
  }
}
