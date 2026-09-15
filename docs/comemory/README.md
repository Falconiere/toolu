# comemory migration

The `comemory@toolu` plugin has been retired. Host integration is maintained by
[Falconiere/comemory](https://github.com/Falconiere/comemory).

Obtain a current `comemory` binary and verify `comemory install --help`. Install
the new integration before removing the legacy plugin:

```bash
comemory install claude
# or
comemory install codex
```

After installation succeeds, remove the legacy plugin:

```text
/plugin uninstall comemory@toolu
codex plugin remove comemory@toolu
```

Existing comemory data and `.toolu/skills` remain in place.
