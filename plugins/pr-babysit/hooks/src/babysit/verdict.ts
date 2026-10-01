import { createHash } from "node:crypto";

export interface ReviewFinding {
  path: string;
  line: number | null;
  severity: string;
  text: string;
  key: string;
}

export interface ReviewVerdict {
  is_review_comment: boolean;
  state: "unknown" | "in_progress" | "complete" | "provider_error";
  complete: boolean;
  verdict: "none" | "approved" | "changes";
  verdict_label: string;
  findings: ReviewFinding[];
  must_fix?: string[];
}

const EMPTY: ReviewVerdict = {
  is_review_comment: false,
  state: "unknown",
  complete: false,
  verdict: "none",
  verdict_label: "",
  findings: [],
};

/** Parse one recorded CI review comment. Keeps the Bash command's JSON contract. */
export function parseVerdict(input: string): ReviewVerdict {
  if (!input.trim()) return { ...EMPTY, findings: [] };

  const lines = input.split(/\n/);
  const reviewMarker = lines.some((line) =>
    /^### Code Review|^### PR Review in Progress|\[View job\]\([^)]*actions\/runs\/[0-9]+|`agent-merge-[a-z]|^`(?:merge-approved|request-changes)`$/.test(
      line,
    ),
  );
  if (!reviewMarker) return { ...EMPTY, findings: [] };

  const unchecked = lines.some((line) => /^\s*-\s*\[\s*\]/.test(line));
  const checked = lines.some((line) => /^\s*-\s*\[[xX]\]/.test(line));
  let state: ReviewVerdict["state"] =
    !checked && !unchecked ? "unknown" : unchecked ? "in_progress" : "complete";
  let complete = state === "complete";

  // The checklist label is emitted before findings that may quote other labels.
  let verdictLabel = input.match(/Set verdict label \(`([^`]+)`\)/)?.[1] ?? "";
  if (!verdictLabel) {
    verdictLabel =
      lines
        .flatMap((line) => {
          const match = line.match(/^`(agent-merge-[a-z-]+|merge-approved|request-changes)`$/);
          return match ? [match[1]!] : [];
        })
        .at(-1) ?? "";
  }
  if (!verdictLabel) verdictLabel = input.match(/agent-merge-[a-z-]+/)?.[0] ?? "";

  let verdict: ReviewVerdict["verdict"];
  if (verdictLabel.includes("approved")) verdict = "approved";
  else if (verdictLabel.includes("blocked") || verdictLabel.includes("changes"))
    verdict = "changes";
  else {
    const summary = lines.find((line) => /^\*\*Verdict:\*\*/.test(line)) ?? "";
    if (/changes requested/i.test(summary)) verdict = "changes";
    else if (/approved/i.test(summary)) verdict = "approved";
    else if (/\*\*Changes requested\*\*|changes-requested/i.test(input)) verdict = "changes";
    else if (/\*\*Approved\*\*/i.test(input)) verdict = "approved";
    else verdict = "none";
  }

  const verdictLine = lines.find((line) => /^\*\*Verdict:\*\*/.test(line)) ?? "";
  if (/review incomplete|provider error/i.test(verdictLine)) {
    state = "provider_error";
    complete = false;
  }

  const findings: ReviewFinding[] = [];
  let inFindings = false;
  for (const line of lines) {
    if (/^### Findings(?:\s|$)/.test(line)) {
      inFindings = true;
      continue;
    }
    if (/^### /.test(line)) inFindings = false;
    if (!inFindings) continue;
    const match = line.match(/^`([^`]+)`: (blocker|high|medium|low|nit): (.*)$/);
    if (!match) continue;
    const [, rawPath, severity, text] = match;
    const pathLine = rawPath!.match(/^(.+):([0-9]+)$/);
    const path = pathLine?.[1] ?? rawPath!;
    const lineNumber = pathLine ? Number(pathLine[2]) : null;
    const hash = createHash("sha1").update(text!).digest("hex").slice(0, 8);
    findings.push({
      path,
      line: lineNumber,
      severity: severity!,
      text: text!,
      key: `${path}:${pathLine?.[2] ?? ""}:${hash}`,
    });
  }

  const mustFix: string[] = [];
  let inTopN = false;
  for (const line of lines) {
    if (/^###\s+Top-N\s+must-fix(?:\s|$)/i.test(line)) {
      inTopN = true;
      continue;
    }
    if (/^### |^<details>/.test(line)) inTopN = false;
    if (!inTopN) continue;
    let item = line.trim();
    if (!item) continue;
    item = item.replace(/^[-*] /, "").replace(/^[0-9]+\.\s+/, "");
    if (item) mustFix.push(item);
  }

  return {
    is_review_comment: true,
    state,
    complete,
    verdict,
    verdict_label: verdictLabel,
    findings,
    must_fix: mustFix,
  };
}
