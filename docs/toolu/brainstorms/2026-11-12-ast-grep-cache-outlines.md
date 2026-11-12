# Cache outlines and map repositories — brainstorm

**Outcome:** Repeated file outlines use a shared content-addressed cache; large repository outlines summarize every supported file by directory.

**Evidence:** Issue #859 pins the cache inside `toolu_engine::outline::outline`, Git common-dir storage, `hash-object --stdin-paths`, atomic writes, and a 14-day/20,000-entry sweep. Merged #857 provides the outline API, batched scans, and a 150-file map. `map::listed` currently requires Git, so a directory outside Git cannot meet #859's acceptance criterion.

**Decisions:** Keep #857's file map for at most 150 files. For larger directories, aggregate all supported files by containing directory, with bounded output. Use Git's listing in repositories and a filesystem walk only outside Git. Hash the working-tree bytes with `git hash-object --no-filters --stdin-paths`; paths that cannot be sent safely are uncached. Treat cache read/write failures as misses/uncached work. A cache entry contains no path, so worktrees reuse it.

**Alternatives:** Keeping the first 150 file rows would not map the whole repository. Replacing Git enumeration with filesystem walking would change ignore behavior in repositories.

**Risk:** Git filenames containing line breaks need an uncached fallback; the hash-object stdin format is line-based. Very large maps still obey the existing 12 KiB text cap.

**Handoff:** Write the spec, then plan real Git and ast-grep checks.
