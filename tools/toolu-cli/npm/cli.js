#!/usr/bin/env node
// `@toolu/plugins` is deprecated. This file never runs a binary.

const NOTICE = `@toolu/plugins is deprecated. Install the native binary, then install plugins with it:
  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash
  brew install falconiere/tap/toolu
  toolu plugins install
See https://github.com/Falconiere/toolu/blob/main/docs/install.md
`;

process.stderr.write(NOTICE);
process.exit(69);
