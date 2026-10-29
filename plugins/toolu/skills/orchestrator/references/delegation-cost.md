# When delegation pays

| Situation | Decision |
| --- | --- |
| Read forty files to return an ownership map | Delegate to isolate the read; request paths and conclusions. |
| Investigate independent subsystems | Parallelize with disjoint scopes and a concurrency cap. |
| Known file or single command | Inline: the brief and round trip cost more than the task. |
| A → B → C where each needs the previous result | Inline; parallelize only independent branches. |
| Several reviewers on one small file | Usually duplicate effort; use one bounded review. |
| A summary would force you to reread everything | Improve its evidence contract or work inline. |

Account for prompt preparation, launch latency, summary omissions and integration
checks. Task size alone does not justify delegation. Do not promise a speedup
without measurements; a shared test/build bottleneck remains shared.

Operational rules and agent selection live in [the orchestrator](../SKILL.md).
