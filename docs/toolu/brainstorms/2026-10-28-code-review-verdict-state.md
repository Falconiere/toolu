# Code-review verdict and state

**Outcome:** `toolu code-review run` reports a compact, derived verdict, persists branch memory, and writes gate-compatible push-review state only after complete committed-diff coverage.

**Decision:** Move the existing v2 writer into `toolu-state` and call it from both review plugins. Keep branch review memory in a separate locked file under the host state root. Convert the shaper's raw path names to Git's quoted `--name-only` spelling for the gate.

**Evidence:** #669 and epic #660 define severity, memory, consent, and exit behavior. `crates/toolu-review/src/state.rs` is the existing v2 writer; `crates/code-review/src/shape.rs` produces raw NUL paths; `crates/core/engine/src/verdict/review.rs` compares text-mode Git names. Jev selected the shared-writer approach over a subprocess or duplicated writer.

**Risk:** The two path spellings diverge for unusual names. A real-Git test must prove a Unicode path passes the gate. Concurrent state writes need the existing lock protocol. `toolu-review` remains the last writer when it runs afterward.

**Scope:** The local report, state, skill, docs, and required tests. GitHub comment posting and automatic fixes remain outside this issue.
