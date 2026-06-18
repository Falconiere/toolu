# Changelog

## [1.23.0](https://github.com/Falconiere/toolu/compare/v1.22.0...v1.23.0) (2026-06-18)


### Features

* **statusline:** show git working-tree status and ahead/behind in statusline ([a3915f0](https://github.com/Falconiere/toolu/commit/a3915f037beeb2f942da4ac40475cbb5c394a2fd))


### Bug Fixes

* **statusline:** use explicit -b main in ahead/behind tests for CI compat ([c4c5ef7](https://github.com/Falconiere/toolu/commit/c4c5ef7ba076af2cb53bb222e82bd941b11a3244))

## [1.22.0](https://github.com/Falconiere/toolu/compare/v1.21.0...v1.22.0) (2026-06-18)


### Features

* **design:** add design plugin with stack-aware design-review skill ([5fc238c](https://github.com/Falconiere/toolu/commit/5fc238c3d448f836939f75181a904eb5ae1c99dd))

## [1.21.0](https://github.com/Falconiere/toolu/compare/v1.20.0...v1.21.0) (2026-06-17)


### Features

* **jira:** nudge Claude to the jira skill over the Atlassian MCP ([10f06ae](https://github.com/Falconiere/toolu/commit/10f06aee0320e998eb8cc28cf565134996a552a6))

## [1.20.0](https://github.com/Falconiere/toolu/compare/v1.19.0...v1.20.0) (2026-06-17)


### Features

* **opencode:** add opencode adapter, install script, and opencode-format agents/commands ([af54ca9](https://github.com/Falconiere/toolu/commit/af54ca96bd56e5a79b784da19a6f8f01188df348))
* **tooling:** --dry-run plan, auto-draft release notes, wire curated notes into GitHub Release body ([#84](https://github.com/Falconiere/toolu/issues/84)) ([3943741](https://github.com/Falconiere/toolu/commit/3943741eb8dd22c18f730c0c59fdcb5f80deab07))
* **tooling:** automate releases with release-please ([29fc41f](https://github.com/Falconiere/toolu/commit/29fc41f8c1ac54baf3b504c17047f01dbf081c39))


### Bug Fixes

* **opencode:** address review comments on the opencode port ([1706d44](https://github.com/Falconiere/toolu/commit/1706d44571c3a8bc1e496105c5553742943c229a))
* **opencode:** guard unbound TOOLU_CONFIG_DIR in install script ([6111a7a](https://github.com/Falconiere/toolu/commit/6111a7a584bd3166a988f3810de55c0593f412af))
* **release:** check out repo so the package.json sync step has files ([fce0dc4](https://github.com/Falconiere/toolu/commit/fce0dc41465bc86684eea7115e67523cc2941c53))
* **release:** switch release-please to whole-repo single-version mode ([31b9d8a](https://github.com/Falconiere/toolu/commit/31b9d8ad543de8dda6602cd2a77d42f30f2ae12c))
