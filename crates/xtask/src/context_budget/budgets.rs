//! Budget data: which files and descriptions `context-budget` checks, and their ceilings.

/// Word ceilings for the session docs under `plugins/toolu/hooks/docs/`.
pub(super) const DOC_BUDGETS: &[(&str, u32)] = &[
  ("session-start", 110),
  ("model-routing", 90),
  ("post-compaction", 28),
  ("session-start-ts", 30),
  ("session-start-rust", 36),
  ("session-start-python", 36),
];

/// Whole-file word ceilings (name, path, words) for files a host loads on invoke.
pub(super) const FILE_BUDGETS: &[(&str, &str, u32)] = &[
  (
    "test-writer body",
    "plugins/toolu/skills/test-writer/SKILL.md",
    960,
  ),
  (
    "test-writer rust",
    "plugins/toolu/skills/test-writer/references/rust.md",
    275,
  ),
  (
    "test-writer python",
    "plugins/toolu/skills/test-writer/references/python.md",
    185,
  ),
  (
    "test-writer typescript-react",
    "plugins/toolu/skills/test-writer/references/typescript-react.md",
    320,
  ),
  (
    "test-writer e2e-web",
    "plugins/toolu/skills/test-writer/references/e2e-web.md",
    235,
  ),
  (
    "test-writer e2e-mobile",
    "plugins/toolu/skills/test-writer/references/e2e-mobile.md",
    300,
  ),
  (
    "test-author agent",
    "plugins/toolu/agents/test-author.md",
    240,
  ),
  (
    "test-writer command",
    "plugins/toolu/commands/test-writer.md",
    25,
  ),
];

/// A skill whose frontmatter description is budgeted and must carry `phrases`.
pub(super) struct Skill {
  pub(super) name: &'static str,
  pub(super) path: &'static str,
  pub(super) budget: u32,
  pub(super) phrases: &'static [&'static str],
}

/// Budgeted skill descriptions.
pub(super) const SKILLS: &[Skill] = &[
  Skill {
    name: "git-better",
    path: "plugins/git-better/skills/git-better/SKILL.md",
    budget: 60,
    phrases: &["write a commit message", "name a branch", "Intent:"],
  },
  Skill {
    name: "sandbox",
    path: "plugins/sandbox/skills/sandbox/SKILL.md",
    budget: 60,
    phrases: &["run the gate in the sandbox", "test on Linux"],
  },
  Skill {
    name: "delivery-flow",
    path: "plugins/delivery-flow/skills/delivery-flow/SKILL.md",
    budget: 50,
    phrases: &["implement and deliver", "real-data execution", "PR"],
  },
  Skill {
    name: "brainstorm",
    path: "plugins/brainstorm/skills/brainstorm/SKILL.md",
    budget: 50,
    phrases: &["brainstorm", "trade-offs"],
  },
  Skill {
    name: "deep-research",
    path: "plugins/toolu/skills/deep-research/SKILL.md",
    budget: 80,
    phrases: &["deep research", "cited report"],
  },
  Skill {
    name: "test-writer",
    path: "plugins/toolu/skills/test-writer/SKILL.md",
    budget: 50,
    phrases: &["write tests", "TDD"],
  },
  Skill {
    name: "debug",
    path: "plugins/toolu/skills/debug/SKILL.md",
    budget: 40,
    phrases: &["root cause", "flaky"],
  },
  Skill {
    name: "orchestrator",
    path: "plugins/toolu/skills/orchestrator/SKILL.md",
    budget: 40,
    phrases: &["delegate"],
  },
  Skill {
    name: "toolu-review",
    path: "plugins/toolu-review/skills/review/SKILL.md",
    budget: 65,
    phrases: &["review before push"],
  },
  Skill {
    name: "epic-orchestrator",
    path: "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md",
    budget: 80,
    phrases: &["epic", "dry-run the epic", "where is the epic at"],
  },
  Skill {
    name: "ast-grep",
    path: "plugins/ast-grep/skills/ast-grep/SKILL.md",
    budget: 40,
    phrases: &[],
  },
  Skill {
    name: "jev",
    path: "plugins/jev/skills/jev/SKILL.md",
    budget: 60,
    phrases: &[],
  },
];
