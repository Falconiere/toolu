/** Jira's four operations for epic-orchestrator, using environment-only auth. */
import { parseJson, send } from "@toolu/core/rest";
import { defaultPolicy, withRetry } from "../ratelimit.ts";

export type JiraStatus = { statusCategory?: { key?: string } };
type JiraLinked = { key: string; fields?: { status?: JiraStatus } };
export type JiraLink = {
  type: { inward: string; outward: string };
  inwardIssue?: JiraLinked;
  outwardIssue?: JiraLinked;
};
type JiraIssue = {
  key: string;
  fields: {
    summary?: string;
    status?: JiraStatus;
    issuelinks?: JiraLink[];
    description?: unknown;
    labels?: string[];
  };
};
type JiraTransition = { id: string; name: string; to?: JiraStatus };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStatus(value: unknown): value is JiraStatus {
  return (
    isRecord(value) &&
    (value.statusCategory === undefined ||
      (isRecord(value.statusCategory) &&
        (value.statusCategory.key === undefined || typeof value.statusCategory.key === "string")))
  );
}

function isLinked(value: unknown): value is JiraLinked {
  return (
    isRecord(value) &&
    typeof value.key === "string" &&
    (value.fields === undefined ||
      (isRecord(value.fields) &&
        (value.fields.status === undefined || isStatus(value.fields.status))))
  );
}

function isLink(value: unknown): value is JiraLink {
  return (
    isRecord(value) &&
    isRecord(value.type) &&
    typeof value.type.inward === "string" &&
    typeof value.type.outward === "string" &&
    (value.inwardIssue === undefined || isLinked(value.inwardIssue)) &&
    (value.outwardIssue === undefined || isLinked(value.outwardIssue))
  );
}

function isIssue(value: unknown): value is JiraIssue {
  if (!isRecord(value) || typeof value.key !== "string" || !isRecord(value.fields)) return false;
  const fields = value.fields;
  return (
    (fields.summary === undefined || typeof fields.summary === "string") &&
    (fields.status === undefined || isStatus(fields.status)) &&
    (fields.issuelinks === undefined ||
      (Array.isArray(fields.issuelinks) && fields.issuelinks.every(isLink))) &&
    (fields.labels === undefined ||
      (Array.isArray(fields.labels) && fields.labels.every((label) => typeof label === "string")))
  );
}

function isTransition(value: unknown): value is JiraTransition {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.name === "string" &&
    (value.to === undefined || isStatus(value.to))
  );
}

function parseIssue(value: unknown): JiraIssue {
  if (!isIssue(value)) throw new Error("invalid Jira issue response");
  return value;
}

function parseIssues(value: unknown): JiraIssue[] {
  if (!isRecord(value) || !Array.isArray(value.issues) || !value.issues.every(isIssue)) {
    throw new Error("invalid Jira search response");
  }
  return value.issues;
}

const FIELDS = ["summary", "status", "issuelinks", "description", "labels"];
const SETUP = "Jira requires JIRA_BASE_URL and either JIRA_PAT or JIRA_EMAIL with JIRA_API_TOKEN";

export class JiraClient {
  readonly base: string;
  readonly version: "2" | "3";
  private readonly authorization: string;

  constructor(env: NodeJS.ProcessEnv = process.env) {
    const base = env.JIRA_BASE_URL?.replace(/\/+$/, "") ?? "";
    const pat = env.JIRA_PAT;
    const basic = env.JIRA_EMAIL && env.JIRA_API_TOKEN;
    if (!base || (!pat && !basic)) throw new Error(SETUP);
    const parsed = new URL(base);
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      throw new Error("JIRA_BASE_URL must use http or https");
    }
    const version = env.JIRA_API_VERSION ?? "3";
    if (version !== "2" && version !== "3") {
      throw new Error("JIRA_API_VERSION must be 2 or 3");
    }
    this.base = base;
    this.version = version;
    this.authorization = pat
      ? `Bearer ${pat}`
      : `Basic ${Buffer.from(`${env.JIRA_EMAIL}:${env.JIRA_API_TOKEN}`).toString("base64")}`;
  }

  private path(suffix: string): string {
    return `/rest/api/${this.version}/${suffix}`;
  }

  private async request(
    method: string,
    path: string,
    body?: unknown,
    write = false,
  ): Promise<string> {
    const headers: Record<string, string> = {
      Accept: "application/json",
      Authorization: this.authorization,
    };
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return withRetry(
      () =>
        send("epic jira", {
          url: this.base + path,
          method,
          headers,
          ...(body === undefined ? {} : { body }),
        }),
      { ...defaultPolicy(), retryTransient: !write },
    );
  }

  private async json(method: string, path: string, body?: unknown): Promise<unknown> {
    const text = await this.request(method, path, body);
    return parseJson("epic jira", text);
  }

  rawIssue(key: string): Promise<unknown> {
    return this.json("GET", this.path(`issue/${encodeURIComponent(key)}`));
  }

  async getIssue(key: string, fields?: readonly string[]): Promise<JiraIssue> {
    const selected = fields?.length ? `?fields=${fields.join(",")}` : "";
    return parseIssue(
      await this.json("GET", this.path(`issue/${encodeURIComponent(key)}${selected}`)),
    );
  }

  search(jql: string): Promise<JiraIssue[]> {
    return this.searchPage(jql, [], undefined, 0);
  }

  private async searchPage(
    jql: string,
    issues: JiraIssue[],
    token: string | undefined,
    page: number,
  ): Promise<JiraIssue[]> {
    if (page >= 20) return issues;
    if (this.version === "2") {
      const result = await this.json("POST", this.path("search"), {
        jql,
        fields: FIELDS,
        maxResults: 100,
        startAt: issues.length,
      });
      if (!isRecord(result) || typeof result.total !== "number") {
        throw new Error("invalid Jira search response");
      }
      const pageIssues = parseIssues(result);
      const next = [...issues, ...pageIssues];
      return next.length >= result.total || pageIssues.length === 0
        ? next
        : this.searchPage(jql, next, undefined, page + 1);
    }
    const result = await this.json("POST", this.path("search/jql"), {
      jql,
      fields: FIELDS,
      maxResults: 100,
      ...(token ? { nextPageToken: token } : {}),
    });
    if (
      !isRecord(result) ||
      (result.nextPageToken !== undefined && typeof result.nextPageToken !== "string")
    ) {
      throw new Error("invalid Jira search response");
    }
    const next = [...issues, ...parseIssues(result)];
    return result.nextPageToken ? this.searchPage(jql, next, result.nextPageToken, page + 1) : next;
  }

  async transitions(key: string): Promise<JiraTransition[]> {
    const result = await this.json(
      "GET",
      this.path(`issue/${encodeURIComponent(key)}/transitions`),
    );
    if (
      !isRecord(result) ||
      !Array.isArray(result.transitions) ||
      !result.transitions.every(isTransition)
    ) {
      throw new Error("invalid Jira transitions response");
    }
    return result.transitions;
  }

  async transition(key: string, name: string): Promise<void> {
    const matches = (await this.transitions(key)).filter(
      (item) => item.name.toLowerCase() === name.toLowerCase(),
    );
    if (matches.length !== 1) {
      throw new Error(`Jira transition "${name}" matched ${matches.length} transitions`);
    }
    await this.request(
      "POST",
      this.path(`issue/${encodeURIComponent(key)}/transitions`),
      { transition: { id: matches[0]?.id } },
      true,
    );
  }

  async comment(key: string, text: string): Promise<void> {
    const content =
      this.version === "2"
        ? text
        : {
            type: "doc",
            version: 1,
            content: [{ type: "paragraph", content: [{ type: "text", text }] }],
          };
    await this.request(
      "POST",
      this.path(`issue/${encodeURIComponent(key)}/comment`),
      { body: content },
      true,
    );
  }
}
