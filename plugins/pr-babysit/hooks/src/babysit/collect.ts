import { BabysitError, utcNow } from "./common";
import { GhError, ghJson, ghRun, type GhRunOptions } from "./gh";
import { parseVerdict } from "./verdict";

type Obj = Record<string, any>;
export type GhTransport = (args: string[], options?: GhRunOptions) => Promise<string>;
export type CollectOptions = {
  repo: string;
  pr: number;
  pageSize?: number;
  timeoutSeconds?: number;
  gh?: GhTransport;
  now?: () => string;
};

const PR_FIELDS =
  "number,title,url,author,state,baseRefName,headRefName,headRefOid,statusCheckRollup,mergeable,reviewDecision";
const THREADS_QUERY = `query($owner:String!,$repo:String!,$number:Int!,$pageSize:Int!,$endCursor:String){
  repository(owner:$owner,name:$repo){ pullRequest(number:$number){
    reviewThreads(first:$pageSize,after:$endCursor){
      pageInfo{hasNextPage endCursor}
      nodes{ id isResolved isOutdated path line
        comments(first:$pageSize){ pageInfo{hasNextPage endCursor}
          nodes{ id databaseId body author{login __typename} createdAt url } } } } } } }`;
const COMMENTS_QUERY = `query($id:ID!,$pageSize:Int!,$endCursor:String){
  node(id:$id){ ... on PullRequestReviewThread {
    comments(first:$pageSize,after:$endCursor){ pageInfo{hasNextPage endCursor}
      nodes{ id databaseId body author{login __typename} createdAt url } } } } }`;

const obj = (value: unknown): Obj => (value && typeof value === "object" ? (value as Obj) : {});
const arr = (value: unknown): Obj[] => (Array.isArray(value) ? (value as Obj[]) : []);
const n = (value: unknown): unknown => value ?? null;

function threadComment(raw: Obj): Obj {
  return {
    id: n(raw.id),
    databaseId: n(raw.databaseId),
    body: n(raw.body),
    author: n(obj(raw.author).login),
    authorType: n(obj(raw.author).__typename),
    createdAt: n(raw.createdAt),
    url: n(raw.url),
  };
}

export function normalizeThreads(pages: unknown): Obj[] {
  return arr(pages)
    .flatMap((page) => arr(obj(obj(obj(page).data).repository).pullRequest?.reviewThreads?.nodes))
    .map((raw) => ({
      id: n(raw.id),
      isResolved: n(raw.isResolved),
      isOutdated: n(raw.isOutdated),
      path: n(raw.path),
      line: n(raw.line),
      comments: arr(obj(raw.comments).nodes).map(threadComment),
      commentsHasNextPage: n(obj(obj(raw.comments).pageInfo).hasNextPage),
      commentsEndCursor: n(obj(obj(raw.comments).pageInfo).endCursor),
    }));
}

export function normalizeThreadComments(pages: unknown): Obj[] {
  return arr(pages)
    .flatMap((page) => arr(obj(obj(page).data).node?.comments?.nodes))
    .map(threadComment);
}

export function normalizeComments(pages: unknown): Obj[] {
  return arr(pages)
    .flatMap((page) => arr(page))
    .map((raw) => ({
      id: n(raw.id),
      body: n(raw.body),
      author: n(obj(raw.user).login),
      authorType: n(obj(raw.user).type),
      createdAt: n(raw.created_at),
      updatedAt: n(raw.updated_at),
      url: n(raw.html_url),
    }));
}

export function normalizeReviews(pages: unknown): Obj[] {
  return arr(pages)
    .flatMap((page) => arr(page))
    .map((raw) => ({
      id: n(raw.id),
      state: n(raw.state),
      body: n(raw.body),
      author: n(obj(raw.user).login),
      authorType: n(obj(raw.user).type),
      submittedAt: n(raw.submitted_at),
      url: n(raw.html_url),
      commitId: n(raw.commit_id),
    }));
}

const CI_REVIEWERS = new Set(["github-actions", "github-actions[bot]", "claude", "claude[bot]"]);
export function isCiReviewer(login: unknown): boolean {
  return typeof login === "string" && CI_REVIEWERS.has(login);
}

function botComment(comments: Obj[]): Obj {
  const candidates = comments.filter((comment) => isCiReviewer(comment.author)).reverse();
  let chosen: Obj | undefined;
  let verdict: Obj | undefined;
  for (const comment of candidates) {
    const parsed = parseVerdict(String(comment.body ?? "")) as unknown as Obj;
    if (parsed.is_review_comment === true) {
      chosen = comment;
      verdict = parsed;
      break;
    }
  }
  if (!chosen) {
    chosen = candidates[0];
    verdict = parseVerdict(String(chosen?.body ?? "")) as unknown as Obj;
    if (!chosen) verdict.state = "absent";
  }
  return {
    comment: chosen
      ? {
          id: chosen.id,
          url: chosen.url,
          createdAt: chosen.createdAt,
          updatedAt: chosen.updatedAt,
          author: chosen.author,
        }
      : null,
    verdict,
  };
}

function readError(source: string, error: unknown, prefix = "collect-pr.sh: read"): never {
  if (error instanceof BabysitError) throw error;
  if (error instanceof GhError) {
    throw new BabysitError("api_error", `${prefix} '${source}' failed: ${error.lastMessage}`, {
      source,
      attempts: error.attempts,
      class: error.classification,
      lastMessage: error.lastMessage,
    });
  }
  throw new BabysitError(
    "invalid_json",
    `${prefix} '${source}' failed: response is not valid JSON or carries GraphQL errors[]`,
    {
      source,
      attempts: 1,
      class: "invalid_json",
      lastMessage: "response is not valid JSON or carries GraphQL errors[]",
    },
  );
}

export async function collectPr(options: CollectOptions): Promise<Obj> {
  const { repo, pr, pageSize = 100, timeoutSeconds, gh = ghRun, now = utcNow } = options;
  const [owner, name] = repo.split("/");
  const transport = (args: string[]) =>
    gh(args, timeoutSeconds === undefined ? {} : { timeoutSeconds });
  const read = async (source: string, args: string[]): Promise<unknown> => {
    try {
      return ghJson(await transport(args));
    } catch (error) {
      return readError(source, error);
    }
  };
  const head = async (): Promise<string> => {
    let doc: Obj;
    try {
      doc = obj(
        ghJson(await transport(["pr", "view", String(pr), "--repo", repo, "--json", "headRefOid"])),
      );
    } catch (error) {
      if (error instanceof GhError)
        throw new BabysitError(
          "api_error",
          `gh head failed after ${error.attempts} attempt(s): ${error.lastMessage || `rc ${error.lastRc}`}`,
          {
            source: "head",
            attempts: error.attempts,
            class: error.classification,
            lastMessage: error.lastMessage,
          },
        );
      throw new BabysitError("invalid_json", "collect-pr.sh: head read returned no headRefOid", {
        source: "head",
      });
    }
    if (typeof doc.headRefOid !== "string" || doc.headRefOid === "")
      throw new BabysitError("invalid_json", "collect-pr.sh: head read returned no headRefOid", {
        source: "head",
      });
    return doc.headRefOid;
  };

  let recollected = false;
  for (let round = 0; round < 2; round += 1) {
    const before = await head();
    const reads: Array<[string, string[]]> = [
      ["pr", ["pr", "view", String(pr), "--repo", repo, "--json", PR_FIELDS]],
      [
        "threads",
        [
          "api",
          "graphql",
          "--paginate",
          "--slurp",
          "-f",
          `owner=${owner}`,
          "-f",
          `repo=${name}`,
          "-F",
          `number=${pr}`,
          "-F",
          `pageSize=${pageSize}`,
          "-f",
          `query=${THREADS_QUERY}`,
        ],
      ],
      [
        "comments",
        [
          "api",
          "--paginate",
          "--slurp",
          `repos/${repo}/issues/${pr}/comments?per_page=${pageSize}`,
        ],
      ],
      [
        "reviews",
        ["api", "--paginate", "--slurp", `repos/${repo}/pulls/${pr}/reviews?per_page=${pageSize}`],
      ],
    ];
    // Start every read before observing a failure, preserving the shell fan-out.
    const settled = await Promise.allSettled(reads.map(([source, args]) => read(source, args)));
    for (let i = 0; i < settled.length; i += 1) {
      const outcome = settled[i]!;
      if (outcome.status === "rejected") throw outcome.reason;
    }
    const [prRaw, threadPages, commentPages, reviewPages] = settled.map(
      (outcome) => (outcome as PromiseFulfilledResult<unknown>).value,
    );
    const threads = normalizeThreads(threadPages);
    let threadCommentPages = 0;
    for (const thread of threads) {
      if (thread.commentsHasNextPage !== true) continue;
      const pages = await read("threadComments", [
        "api",
        "graphql",
        "--paginate",
        "--slurp",
        "-f",
        `id=${thread.id}`,
        "-F",
        `pageSize=${pageSize}`,
        "-f",
        `endCursor=${thread.commentsEndCursor}`,
        "-f",
        `query=${COMMENTS_QUERY}`,
      ]);
      threadCommentPages += arr(pages).length;
      thread.comments.push(...normalizeThreadComments(pages));
      thread.commentsHasNextPage = false;
      thread.commentsEndCursor = null;
    }
    const comments = normalizeComments(commentPages);
    const reviews = normalizeReviews(reviewPages);
    const bot = botComment(comments);
    const after = await head();
    if (before !== after) {
      recollected = true;
      continue;
    }
    const raw = obj(prRaw);
    return {
      version: 1,
      collectedAt: now(),
      repo,
      number: pr,
      head: { sha: after, verifiedAfterFanout: true, recollected },
      pageSize,
      pr: {
        number: n(raw.number),
        title: n(raw.title),
        url: n(raw.url),
        author: n(obj(raw.author).login),
        state: n(raw.state),
        baseRefName: n(raw.baseRefName),
        headRefName: n(raw.headRefName),
        headRefOid: n(raw.headRefOid),
        mergeable: n(raw.mergeable),
        reviewDecision: n(raw.reviewDecision),
        statusCheckRollup: raw.statusCheckRollup ?? [],
      },
      threads: threads.map(
        ({ commentsHasNextPage: _hasNext, commentsEndCursor: _cursor, ...thread }) => thread,
      ),
      comments,
      reviews,
      bot,
      pages: {
        threads: arr(threadPages).length,
        threadComments: threadCommentPages,
        comments: arr(commentPages).length,
        reviews: arr(reviewPages).length,
      },
    };
  }
  throw new BabysitError("head_moved", "collect-pr.sh: PR head moved twice during collection", {
    source: "head",
  });
}
