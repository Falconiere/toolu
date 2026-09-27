/** Tracker detection and the Jira/Linear payload normalizers, on API-shaped JSON. */

import { describe, expect, test } from "bun:test";
import { branchFor, keyFor } from "../epic-graph.ts";
import { detectTracker } from "../trackers/index.ts";
import { adfText, jiraBlockers, parseJiraRef } from "../trackers/jira.ts";
import { linearBlockers, parseLinearRef } from "../trackers/linear.ts";
import { repoFor } from "../trackers/types.ts";

describe("detectTracker", () => {
  // A HOME without a jira CLI login, so only the env decides.
  const none = { HOME: "/nonexistent" };
  test("GitHub refs", () => {
    expect(detectTracker("https://github.com/a/b/issues/3", undefined, none)).toBe("github");
    expect(detectTracker("a/b#3", undefined, none)).toBe("github");
    expect(detectTracker("#3", undefined, none)).toBe("github");
  });

  test("URLs and prefixes are unambiguous", () => {
    expect(detectTracker("https://acme.atlassian.net/browse/PAY-7", undefined, none)).toBe("jira");
    expect(detectTracker("jira:PAY-7", undefined, none)).toBe("jira");
    expect(detectTracker("https://linear.app/acme/issue/ENG-9/title", undefined, none)).toBe(
      "linear",
    );
    expect(detectTracker("linear:ENG-9", undefined, none)).toBe("linear");
  });

  test("a bare key follows the configured tracker, else asks", () => {
    expect(detectTracker("PAY-7", undefined, { ...none, LINEAR_API_KEY: "k" })).toBe("linear");
    expect(() =>
      detectTracker("PAY-7", undefined, {
        ...none,
        JIRA_BASE_URL: "https://x",
        LINEAR_API_KEY: "k",
      }),
    ).toThrow("prefix it");
    expect(detectTracker("PAY-7", "jira", none)).toBe("jira");
  });
});

describe("refs", () => {
  test("Jira keys and browse URLs", () => {
    expect(parseJiraRef("https://acme.atlassian.net/browse/PAY-12?focused=1")).toBe("PAY-12");
    expect(parseJiraRef("jira:pay-12")).toBe("PAY-12");
    expect(parseJiraRef("12")).toBeNull();
  });

  test("Linear issues and projects", () => {
    expect(parseLinearRef("https://linear.app/acme/issue/ENG-42/fix-login")).toEqual({
      kind: "issue",
      id: "ENG-42",
    });
    expect(parseLinearRef("https://linear.app/acme/project/sync-engine-4f2a9c1b8e7d")).toEqual({
      kind: "project",
      id: "4f2a9c1b8e7d",
    });
    expect(parseLinearRef("project:abc123")).toEqual({ kind: "project", id: "abc123" });
  });

  test("keys and branches for Jira/Linear items are valid herdr names", () => {
    const item = { ref: "PAY-12", repo: "acme/pay", number: null, title: "Add refunds API" };
    expect(keyFor(item)).toBe("pay-12");
    expect(branchFor(item)).toBe("feat/pay-12-add-refunds-api");
    expect(keyFor({ ref: "a/b#3", repo: "acme/pay", number: 3 })).toBe("acme-pay-3");
  });
});

describe("Jira normalizers", () => {
  test("blockers from both link directions and link types", () => {
    const done = { statusCategory: { key: "done" } };
    const todo = { statusCategory: { key: "new" } };
    const links = [
      {
        type: { name: "Blocks", inward: "is blocked by", outward: "blocks" },
        inwardIssue: { key: "PAY-1", fields: { status: done } },
      },
      {
        type: { name: "Blocks", inward: "is blocked by", outward: "blocks" },
        outwardIssue: { key: "PAY-9", fields: { status: todo } },
      },
      {
        type: { name: "Dependency", inward: "is depended on by", outward: "depends on" },
        outwardIssue: { key: "PAY-3", fields: { status: todo } },
      },
      {
        type: { name: "Relates", inward: "relates to", outward: "relates to" },
        outwardIssue: { key: "PAY-4", fields: { status: todo } },
      },
    ];
    expect(jiraBlockers(links)).toEqual({ "PAY-1": "closed", "PAY-3": "open" });
  });

  test("ADF description to text", () => {
    const doc = {
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "Repo: acme/api" }] },
        { type: "paragraph", content: [{ type: "text", text: "Second line" }] },
      ],
    };
    expect(adfText(doc)).toContain("Repo: acme/api\n");
    expect(adfText("plain v2 text")).toBe("plain v2 text");
  });
});

test("Linear blockers come from inverse blocks relations only", () => {
  const issue = {
    id: "u1",
    identifier: "ENG-5",
    title: "t",
    url: "u",
    state: { type: "started" },
    inverseRelations: {
      nodes: [
        { type: "blocks", issue: { identifier: "ENG-1", state: { type: "completed" } } },
        { type: "blocks", issue: { identifier: "ENG-2", state: { type: "unstarted" } } },
        { type: "related", issue: { identifier: "ENG-3", state: { type: "unstarted" } } },
      ],
    },
  };
  expect(linearBlockers(issue)).toEqual({ "ENG-1": "closed", "ENG-2": "open" });
});

test("code repo: label, then Repo: line, then default", () => {
  expect(repoFor(["backend", "repo:acme/api"], "Repo: acme/web", "acme/mono")).toBe("acme/api");
  expect(repoFor([], "Context\nRepo: acme/web\n", "acme/mono")).toBe("acme/web");
  expect(repoFor([], "no hint", "acme/mono")).toBe("acme/mono");
});
