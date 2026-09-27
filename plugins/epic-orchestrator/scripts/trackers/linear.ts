/** Linear epics over the GraphQL API (`LINEAR_API_KEY`). An epic is a parent
 * issue (children are its sub-issues) or a project (children are its issues).
 * A blocker is an inverse `blocks` relation. */

import { fetchJson } from "../ratelimit.ts";
import { excerpt, repoFor, type EpicInfo, type TrackedIssue, type Tracker } from "./types.ts";

const API = "https://api.linear.app/graphql";
const ISSUE_URL = /linear\.app\/[^/\s]+\/issue\/([A-Z][A-Z0-9]*-\d+)/i;
const PROJECT_URL = /linear\.app\/[^/\s]+\/project\/([\w-]+)/i;
const IDENT = /^[A-Z][A-Z0-9]*-\d+$/;
const CLOSED_TYPES = new Set(["completed", "canceled"]);

const CHILD_FIELDS = `id identifier title url description state{type}
labels{nodes{name}}
inverseRelations(first:50){nodes{type issue{identifier state{type}}}}`;

type State = { type: string };
type LinearIssue = {
  id: string;
  identifier: string;
  title: string;
  url: string;
  description?: string | null;
  state: State;
  labels?: { nodes: { name: string }[] };
  inverseRelations?: { nodes: { type: string; issue: { identifier: string; state: State } }[] };
};
type Page = { nodes: LinearIssue[]; pageInfo: { hasNextPage: boolean; endCursor: string | null } };

export type LinearRef = { kind: "issue" | "project"; id: string };

export function parseLinearRef(ref: string): LinearRef | null {
  const t = ref.trim().replace(/^linear:/i, "");
  const issue = ISSUE_URL.exec(t);
  if (issue?.[1]) return { kind: "issue", id: issue[1].toUpperCase() };
  const project = PROJECT_URL.exec(t);
  if (project?.[1]) {
    // Project URLs are `<name>-<slugId>`; the slugId is the last non-empty segment.
    const id = project[1].split("-").filter(Boolean).pop();
    if (id) return { kind: "project", id };
  }
  if (/^project[:/]/i.test(t)) return { kind: "project", id: t.replace(/^project[:/]/i, "") };
  return IDENT.test(t.toUpperCase()) ? { kind: "issue", id: t.toUpperCase() } : null;
}

function closed(s: State | undefined): "open" | "closed" {
  return s && CLOSED_TYPES.has(s.type) ? "closed" : "open";
}

/** Linear's two auth schemes: a personal API key (`lin_api_…`) goes in the
 * header bare, an OAuth2 access token (`lin_oauth_…`) as `Bearer <token>`.
 * https://linear.app/developers/graphql#authentication */
export function linearAuthHeader(token: string): string {
  const t = token.trim();
  if (/^bearer\s/i.test(t)) return t;
  return t.startsWith("lin_oauth_") ? `Bearer ${t}` : t;
}

export function linearBlockers(i: LinearIssue): Record<string, string> {
  const out: Record<string, string> = {};
  for (const r of i.inverseRelations?.nodes ?? []) {
    if (r.type === "blocks") out[r.issue.identifier] = closed(r.issue.state);
  }
  return out;
}

export class LinearTracker implements Tracker {
  readonly kind = "linear";
  readonly epicRef: string;
  private readonly ref: LinearRef;
  private readonly auth: string;
  private readonly api: string;

  /** `LINEAR_API_KEY` holds a personal key or an OAuth token;
   * `LINEAR_API_URL` overrides the endpoint (a proxy, or a test server). */
  constructor(
    ref: string,
    private readonly defaultRepo: string,
    env: NodeJS.ProcessEnv = process.env,
  ) {
    const parsed = parseLinearRef(ref);
    if (!parsed) throw new Error(`not a Linear issue identifier or URL: ${ref}`);
    const key = env.LINEAR_API_KEY;
    if (!key) throw new Error("LINEAR_API_KEY is not set");
    this.ref = parsed;
    this.auth = linearAuthHeader(key);
    this.api = env.LINEAR_API_URL || API;
    this.epicRef = parsed.kind === "project" ? `project:${parsed.id}` : parsed.id;
  }

  private async gql<T>(query: string, variables: Record<string, unknown>): Promise<T> {
    const res = (await fetchJson(this.api, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: this.auth },
      body: JSON.stringify({ query, variables }),
    })) as { data?: T; errors?: { message: string }[] };
    if (res.errors?.length)
      throw new Error(`Linear: ${res.errors.map((e) => e.message).join("; ")}`);
    if (!res.data) throw new Error("Linear: empty response");
    return res.data;
  }

  async epic(): Promise<EpicInfo> {
    if (this.ref.kind === "project") {
      const d = await this.gql<{
        project: { id: string; name: string; url: string; completedAt: string | null };
      }>(`query($id:String!){project(id:$id){id name url completedAt}}`, { id: this.ref.id });
      return {
        ref: this.epicRef,
        title: d.project.name,
        state: d.project.completedAt ? "closed" : "open",
        url: d.project.url,
      };
    }
    const d = await this.gql<{ issue: LinearIssue }>(
      `query($id:String!){issue(id:$id){id identifier title url state{type}}}`,
      { id: this.ref.id },
    );
    return {
      ref: this.epicRef,
      title: d.issue.title,
      state: closed(d.issue.state),
      url: d.issue.url,
    };
  }

  async children(): Promise<TrackedIssue[]> {
    const field = this.ref.kind === "project" ? "project" : "issue";
    const conn = this.ref.kind === "project" ? "issues" : "children";
    const nodes: LinearIssue[] = [];
    let after: string | null = null;
    for (let page = 0; page < 20; page++) {
      const d: Record<string, Record<string, Page>> = await this.gql(
        `query($id:String!,$after:String){${field}(id:$id){${conn}(first:100,after:$after){
nodes{${CHILD_FIELDS}} pageInfo{hasNextPage endCursor}}}}`,
        { id: this.ref.id, after },
      );
      const pageData = d[field]?.[conn];
      if (!pageData) break;
      nodes.push(...pageData.nodes);
      if (!pageData.pageInfo.hasNextPage) break;
      after = pageData.pageInfo.endCursor;
    }
    return nodes.map((i) => {
      const labels = (i.labels?.nodes ?? []).map((l) => l.name);
      const body = i.description ?? "";
      return {
        ref: i.identifier,
        title: i.title,
        state: closed(i.state),
        url: i.url,
        repo: repoFor(labels, body, this.defaultRepo),
        number: null,
        blockers: linearBlockers(i),
        prs: [],
        deps_source: "relations",
        labels,
        excerpt: excerpt(body),
      };
    });
  }

  /** Comment on the issue, then move it to its team's first completed state. */
  private async finish(identifier: string, note: string): Promise<string> {
    const d = await this.gql<{
      issue: {
        id: string;
        state: State;
        team: { states: { nodes: { id: string; position: number }[] } };
      };
    }>(
      `query($id:String!){issue(id:$id){id state{type}
team{states(filter:{type:{eq:"completed"}}){nodes{id position}}}}}`,
      { id: identifier },
    );
    await this.gql(
      `mutation($i:String!,$b:String!){commentCreate(input:{issueId:$i,body:$b}){success}}`,
      { i: d.issue.id, b: note },
    );
    if (closed(d.issue.state) === "closed") return "already-done";
    const target = [...d.issue.team.states.nodes].sort((a, b) => a.position - b.position)[0];
    if (!target) return "no-completed-state";
    await this.gql(
      `mutation($id:String!,$s:String!){issueUpdate(id:$id,input:{stateId:$s}){success}}`,
      { id: d.issue.id, s: target.id },
    );
    return "completed";
  }

  closeIssue(identifier: string, note: string): Promise<string> {
    return this.finish(identifier, note);
  }

  tickEpic(): Promise<boolean> {
    return Promise.resolve(false);
  }

  async closeEpic(summary: string): Promise<string> {
    if (this.ref.kind === "project") {
      // Project status is workspace-configured; leave the close to a human.
      return "project-left-open";
    }
    return this.finish(this.ref.id, summary);
  }

  stateSlug(): string {
    return `linear-${this.epicRef.replace(/[:/]/g, "-")}`.toLowerCase();
  }
}
