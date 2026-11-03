# Evidence and worker handoff

Use this for both new epics and updates. Pin the repository's default-branch
commit before reading. Existing issue text is a requirement or historical
claim, not proof of current behavior.

## Read once, resolve once

Start from the owning module, nearby tests and the repository's instructions.
Follow the callers and public surfaces the change actually reaches, including
fixtures, persistence, config, exports and user documentation. Read each
relevant file once at the pinned commit and reuse that evidence across
children. Record shared decisions in the parent and child-specific contracts
in the child. Cite exact paths and lines; label new paths as proposed.

For updates, compare the old base with the new one where available. Check
comments and linked PRs for changed intent and completed work. If the old base
is unavailable, verify current claims directly and record that limitation.
Resolve contradictions from authoritative decisions and observed code; ask
only when the intended outcome cannot be inferred. Do not infer that missing
search results or a closed issue establish implementation or test coverage.

## Ready for a worker

Each child should answer these without another broad research pass:

- What observable result changes, and what is out of scope?
- Which existing pattern and entry points should the worker use?
- Which callers, tests, fixtures and docs must change with it?
- What interface does it consume or provide to siblings, and which work must
  land first? A dependency is required delivery order, not topic similarity.
- What input and expected outcome prove each criterion, including relevant
  error, recovery and compatibility behavior?

Prefer one coherent, independently verifiable PR. Split on a stable interface
or dependency boundary, not an arbitrary file count. Keep coupled changes
together when separating them would leave the repository broken. Identify
shared-file contention before claiming two children can run in parallel.

## Verification contract

Each criterion names a focused command plus the behavior/test it must cover;
an existing broad test command alone does not establish coverage. Distinguish
an existing check from a test the worker must add. Validate command syntax
against the repository's real tools, and report runs separately from plans.
Do not run a full gate merely to author an issue. Workers run focused checks
while editing and the required full gate before delivery.

The handoff carries the pinned base, decisions, criteria and checks into the
worker's delivery workflow. Workers verify drift and reuse this evidence in
their phase artifacts; they still perform required spec/plan reviews and
quality gates. If a sibling changes a contract, reconcile just the affected
decisions and checks instead of repeating the entire discovery phase.
