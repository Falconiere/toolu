# Code review diff shaping

**Outcome:** Shape the committed push diff into complete file entries with real old/new changed-line anchors, a rename map, and explicit coverage of every changed path.

**Evidence:** #662 requires merge-base, noise, rename, deletion, and anchor behavior. #660 requires bounded evidence and a reviewed/skipped/unreviewed partition. `toolu_state::diff_sha::diff_sha` and `toolu_state::git::base_branch` define the gate's diff and default base. #661 supplies the `shape` module, report coverage types, and budget module.

**Decision:** Resolve the base with the gate helper, require a merge-base, then parse Git's committed `base...HEAD` diff with rename detection. Return structured entries and chunk them only at file boundaries. Keep deleted content on the old side and mark removal lines as old-side anchors. Ask Git for the exact gate path set separately so coverage cannot silently omit a path. Jev preferred structured entries over a raw patch (0.99, `jev-1.14.0`).

**Boundary:** This issue shapes evidence; later issues own model calls, validation, reports, and gate state. No user-facing CLI behavior changes here.

**Risk:** Git's text diff can contain unusual filenames and binary output. Use NUL-delimited path metadata, fail closed on parse mismatches, and exercise real repositories in tests.
