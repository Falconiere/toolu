import { expect, test } from "bun:test";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  collectPr,
  isCiReviewer,
  normalizeComments,
  normalizeReviews,
  normalizeThreadComments,
  normalizeThreads,
  type GhTransport,
} from "../babysit/collect";
import { ghClassify, ghRun } from "../babysit/gh";
import { BabysitError } from "../babysit/common";

const ROOT = resolve(import.meta.dir, "../../../scripts/__tests__/fixtures");
const raw = (path: string) => readFileSync(join(ROOT, path), "utf8");
const fixture = (path: string) => JSON.parse(raw(path));
const captured = fixture("snapshots/toolu-115.json");
const pages = {
  threads: raw("gh/pages-115-threads-p2.json"),
  comments: raw("gh/pages-115-comments-p2.json"),
  reviews: raw("gh/pages-115-reviews-p2.json"),
};

function fakeGh(heads: string[] = [captured.head.sha, captured.head.sha]): {
  gh: GhTransport;
  calls: string[][];
} {
  const calls: string[][] = [];
  const gh: GhTransport = async (args) => {
    calls.push(args);
    if (args[0] === "pr" && args.includes("headRefOid") && args.at(-1) === "headRefOid")
      return JSON.stringify({ headRefOid: heads.shift() ?? captured.head.sha });
    if (args[0] === "pr")
      return JSON.stringify({ ...captured.pr, author: { login: captured.pr.author } });
    if (args[1] === "graphql") return pages.threads;
    if (args.at(-1)?.includes("/issues/")) return pages.comments;
    if (args.at(-1)?.includes("/pulls/")) return pages.reviews;
    throw new Error(`unexpected gh arguments: ${args.join(" ")}`);
  };
  return { gh, calls };
}

test("captured pages normalize to the recorded PR snapshot", async () => {
  expect(normalizeThreads(JSON.parse(pages.threads))).toHaveLength(17);
  expect(normalizeComments(JSON.parse(pages.comments))).toHaveLength(3);
  expect(normalizeReviews(JSON.parse(pages.reviews))).toHaveLength(20);
  expect(normalizeThreadComments(fixture("gh/pages-165-thread-comments-p2.json"))).toHaveLength(3);
  const { gh, calls } = fakeGh();
  const actual = await collectPr({
    repo: "Falconiere/toolu",
    pr: 115,
    pageSize: 2,
    gh,
    now: () => captured.collectedAt,
  });
  expect(actual).toEqual(captured);
  expect(calls).toHaveLength(6); // head, four fan-out reads, verified head
});

test("an absent bot comment does not change a later blank bot verdict", async () => {
  const base = fakeGh();
  const collectWithComments = (comments: unknown[]) =>
    collectPr({
      repo: "Falconiere/toolu",
      pr: 115,
      gh: (args, options) =>
        args.at(-1)?.includes("/issues/")
          ? Promise.resolve(JSON.stringify([comments]))
          : base.gh(args, options),
    });

  const absent = await collectWithComments([]);
  expect(absent.bot.comment).toBeNull();
  expect(absent.bot.verdict.state).toBe("absent");

  const blank = await collectWithComments([
    { id: 9001, body: "", user: { login: "github-actions", type: "Bot" } },
  ]);
  expect(blank.bot.comment.id).toBe(9001);
  expect(blank.bot.verdict.state).toBe("unknown");
  expect(blank.bot.verdict.is_review_comment).toBe(false);
  expect(absent.bot.verdict.state).toBe("absent");
});

test("head movement repeats the whole collection exactly once", async () => {
  const { gh, calls } = fakeGh(["old", "new", "new", "new"]);
  const actual = await collectPr({ repo: "Falconiere/toolu", pr: 115, pageSize: 2, gh });
  expect(actual.head.recollected).toBe(true);
  expect(calls).toHaveLength(12);
});

test("overflowed review-thread comments are fetched through node(id:) and appended in order", async () => {
  const threadPages = structuredClone(JSON.parse(pages.threads));
  const first = threadPages[0].data.repository.pullRequest.reviewThreads.nodes[0];
  first.comments.pageInfo = { hasNextPage: true, endCursor: "cursor-1" };
  const originalCount = first.comments.nodes.length;
  const extra = fixture("gh/pages-165-thread-comments-p2.json");
  const base = fakeGh();
  const gh: GhTransport = async (args, options) => {
    if (args[1] === "graphql" && args.some((arg) => arg === `id=${first.id}`))
      return JSON.stringify(extra);
    if (args[1] === "graphql") return JSON.stringify(threadPages);
    return base.gh(args, options);
  };
  const actual = await collectPr({ repo: "Falconiere/toolu", pr: 115, pageSize: 2, gh });
  expect(actual.threads[0].comments).toHaveLength(originalCount + 3);
  expect(
    actual.threads[0].comments.slice(-3).map((comment: { author: string }) => comment.author),
  ).toEqual(["github-actions", "Falconiere", "github-actions"]);
  expect(actual.pages.threadComments).toBe(2);
  expect(actual.threads[0]).not.toHaveProperty("commentsHasNextPage");
});

test("a moving head twice fails with a structured code", async () => {
  const { gh, calls } = fakeGh(["a", "b", "c", "d"]);
  try {
    await collectPr({ repo: "Falconiere/toolu", pr: 115, pageSize: 2, gh });
    throw new Error("expected rejection");
  } catch (error) {
    expect(error).toBeInstanceOf(BabysitError);
    expect((error as BabysitError).code).toBe("head_moved");
  }
  expect(calls).toHaveLength(12);
});

test("CI reviewer membership is exact", () => {
  for (const login of ["github-actions", "github-actions[bot]", "claude", "claude[bot]"])
    expect(isCiReviewer(login)).toBe(true);
  for (const login of ["dependabot", "dependabot[bot]", "Falconiere", "renovate[bot]"])
    expect(isCiReviewer(login)).toBe(false);
});

test("gh error classification matches recorded permanent and transient cases", () => {
  expect(ghClassify(1, raw("gh/404.err"), raw("gh/404.out"))).toBe("permanent");
  expect(ghClassify(1, raw("gh/graphql-notfound.err"), raw("gh/graphql-notfound.out"))).toBe(
    "permanent",
  );
  expect(ghClassify(1, raw("gh/refused.err"), "")).toBe("transient");
  expect(ghClassify(124, "", "")).toBe("transient");
  expect(ghClassify(1, "gh: rate limit exceeded (HTTP 403)", "")).toBe("transient");
});

test("ghRun retries a stub gh on PATH and never invokes a live PR", async () => {
  const dir = mkdtempSync(join(tmpdir(), "pb-gh-stub-"));
  const oldPath = process.env.PATH;
  try {
    const command = join(dir, "gh");
    const count = join(dir, "count");
    writeFileSync(
      command,
      `#!/bin/sh\ncount=$(cat "$PB_STUB_COUNT" 2>/dev/null || echo 0)\ncount=$((count + 1))\nprintf '%s' "$count" > "$PB_STUB_COUNT"\nif [ "$count" -lt 3 ]; then echo 'dial tcp 127.0.0.1:1: connect: connection refused' >&2; exit 1; fi\nprintf '%s' '{"headRefOid":"abc"}'\n`,
    );
    chmodSync(command, 0o755);
    process.env.PATH = `${dir}:${oldPath ?? ""}`;
    process.env.PB_STUB_COUNT = count;
    expect(
      await ghRun(["pr", "view", "1"], { attempts: 3, backoffSeconds: [0, 0], timeoutSeconds: 2 }),
    ).toBe('{"headRefOid":"abc"}');
    expect(readFileSync(count, "utf8")).toBe("3");
  } finally {
    process.env.PATH = oldPath;
    delete process.env.PB_STUB_COUNT;
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the Bun CLI matches the Bash golden snapshot from stubbed recorded gh pages", async () => {
  const dir = mkdtempSync(join(tmpdir(), "pb-collect-parity-"));
  try {
    const command = join(dir, "gh");
    const prRaw = join(dir, "pr.json");
    writeFileSync(prRaw, JSON.stringify({ ...captured.pr, author: { login: captured.pr.author } }));
    writeFileSync(
      command,
      `#!/bin/sh
case " $* " in
  *' --json headRefOid '*) printf '%s\\n' '{"headRefOid":"${captured.head.sha}"}' ;;
  *' --json ${"number,title,url,author,state,baseRefName,headRefName,headRefOid,statusCheckRollup,mergeable,reviewDecision"} '*) cat "$PB_STUB_PR" ;;
  *'graphql'*) cat "$PB_STUB_THREADS" ;;
  *'/issues/'*) cat "$PB_STUB_COMMENTS" ;;
  *'/pulls/'*) cat "$PB_STUB_REVIEWS" ;;
  *) echo "unexpected gh args: $*" >&2; exit 1 ;;
esac
`,
    );
    chmodSync(command, 0o755);
    const env = {
      ...process.env,
      PATH: `${dir}:${process.env.PATH ?? ""}`,
      PB_STUB_PR: prRaw,
      PB_STUB_THREADS: join(ROOT, "gh/pages-115-threads-p2.json"),
      PB_STUB_COMMENTS: join(ROOT, "gh/pages-115-comments-p2.json"),
      PB_STUB_REVIEWS: join(ROOT, "gh/pages-115-reviews-p2.json"),
    };
    const bunOut = join(dir, "bun.json");
    const args = ["--repo", "Falconiere/toolu", "--pr", "115", "--page-size", "2"];
    const bun = Bun.spawn(
      ["bun", resolve(import.meta.dir, "../babysit-collect-pr.ts"), ...args, "--out", bunOut],
      { env, stdout: "pipe", stderr: "pipe" },
    );
    const bunCode = await bun.exited;
    expect(bunCode).toBe(0);
    // Captured from collect-pr.sh with this stub and these recorded pages.
    const golden = JSON.parse(
      readFileSync(join(import.meta.dir, "fixtures/collect-pr-115-p2.golden.json"), "utf8"),
    );
    const right = JSON.parse(readFileSync(bunOut, "utf8"));
    delete right.collectedAt;
    expect(right).toEqual(golden);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
