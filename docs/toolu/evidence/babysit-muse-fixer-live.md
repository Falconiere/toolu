# Live dispatch-fix round with a Muse fixer (#604)

Run 2026-10-23 on macOS with herdr 0.9.5, Muse Code 1.4.12 and the branch's debug `toolu`.
Scratch repository `/private/tmp/pb604/repo` with a bare `origin`; PR branch `feat/fix` adds a line to a README whose first line reads `helo world`.

## Route (`toolu babysit route-fix --host muse --no-jev`)

```json
{"dispatch":"herdr","groups":[{"class":"implementation","effort":"medium","host":"muse","items":["PRRT_live604a"],"model":"muse-wonder-1.0","seq":1,"tier":"standard"}],"items":[{"confidence":null,"id":"PRRT_live604a","raised":false,"score":null,"source":"heuristic","tier":"standard"}],"note":"jev disabled; heuristic tiers used","source":"heuristic","unattended":true,"version":1}
```

## Start (`toolu babysit dispatch-fix start`)

The same start with `--dry-run`, run against the same state, plan and items afterwards, records the launch command the round used:

```json
["herdr","agent","start","pb-56afc5-r1g1","--kind","muse","--pane","<root pane>","--timeout","90000","--","--yolo","--model","muse-wonder-1.0","--reasoning-effort","medium"]
```

The real start answered:

```json
{"branch":"pr-babysit/scratch-pb604-604","commits":[],"group":1,"groups":[{"agent":"pb-56afc5-r1g1","effort":"medium","host":"muse","model":"muse-wonder-1.0","reason":null,"seq":1,"status":"running","tier":"standard"}],"reason":null,"status":"running","version":1,"worktree":"/Volumes/Projects/.herdr/worktrees/repo/pr-babysit-scratch-pb604-604"}
```

## Wait (`toolu babysit dispatch-fix wait --timeout-seconds 420`)

```json
{"branch":"pr-babysit/scratch-pb604-604","commits":["5798672e0b77f1e530b2dcd4874ba5795f971b43"],"group":1,"groups":[{"agent":"pb-56afc5-r1g1","effort":"medium","host":"muse","model":"muse-wonder-1.0","reason":null,"seq":1,"status":"done","tier":"standard"}],"reason":null,"status":"done","version":1,"worktree":"/Volumes/Projects/.herdr/worktrees/repo/pr-babysit-scratch-pb604-604"}
```

group 1: done. The Muse fixer committed `5798672 fix(readme): address PR review feedback` on the slot branch, changing only README.md line 1 from `helo world` to `hello world`. After the wait no `pb-56afc5-*` agent was listed by `herdr agent list`. The controller step then pushed the slot branch to `origin/feat/fix`.

## Cleanup (`toolu babysit dispatch-fix cleanup`)

```json
{"branchDeleted":true,"note":null,"status":"cleaned","version":1,"worktreeRemoved":true}
```

- agents after cleanup: none (`herdr agent list` has no `pb-56afc5-*` agent and no Muse agent; no `muse` process is running)
- herdr workspace after cleanup: none (no `pb-604` workspace)
- slot worktree after cleanup: none (`git worktree list` shows only the main checkout; the worktree directory is gone)
- slot branch after cleanup: none (`git branch --list 'pr-babysit/*'` is empty)
