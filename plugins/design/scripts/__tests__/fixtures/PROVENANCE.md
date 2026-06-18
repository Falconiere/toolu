# Fixture provenance

Every manifest under this directory is a **verbatim, real-world** file copied
byte-for-byte from a public upstream repository (no mocks, no hand-edited or
synthesized manifests). Each entry records the exact source URL, the commit it
was pinned to, the fetch date, and the upstream license. All sources are under
permissive (MIT / BSD-3-Clause / 0BSD) licenses.

Fetched: 2026-06-18.

## web-next/package.json

- Project: Vercel "Learn Next.js" course — dashboard final example (a real
  Next.js App Router + Tailwind app, the canonical create-next-app-style
  output the official course ships).
- Repo: https://github.com/vercel/next-learn
- Path: `dashboard/final-example/package.json`
- Commit: `31116a0ae378562f0d86000d398804d2fc007986` (committed 2025-12-12)
- Raw URL: https://raw.githubusercontent.com/vercel/next-learn/31116a0ae378562f0d86000d398804d2fc007986/dashboard/final-example/package.json
- License: MIT
- Detector-relevant keys: `dependencies.react`, `dependencies.react-dom`,
  `dependencies.next`, `dependencies.tailwindcss` → expected platform `web`,
  frameworks include `react` + `next`, styling includes `tailwind`.

## mobile-flutter/pubspec.yaml

- Project: flutter/samples — `material_3_demo` (a real, runnable Flutter app).
- Repo: https://github.com/flutter/samples
- Path: `material_3_demo/pubspec.yaml`
- Commit: `2999d738b8c088a1438f9446331a36fc7094ba65` (committed 2025-08-14)
- Raw URL: https://raw.githubusercontent.com/flutter/samples/2999d738b8c088a1438f9446331a36fc7094ba65/material_3_demo/pubspec.yaml
- License: BSD-3-Clause (Chromium-style; permissive)
- Detector-relevant content: the `dependencies: flutter: sdk: flutter` block
  → expected platform `mobile`, `mobile.flutter` true.
- Note: the `flutter create` default template (flutter/flutter
  `packages/flutter_tools/templates/app/pubspec.yaml.tmpl`) was deliberately
  NOT used because it is a Mustache template containing `{{projectName}}` /
  `{{dartSdkVersionBounds}}` placeholders — i.e. not a real, resolved
  manifest. The spec explicitly permits "the default flutter create one, OR
  from flutter/samples"; this is the real flutter/samples option.

## mobile-expo/package.json + mobile-expo/app.json

- Project: expo/expo — `expo-template-default`, the default template that
  `create-expo-app` scaffolds (real, shipped template manifest).
- Repo: https://github.com/expo/expo
- Paths: `templates/expo-template-default/package.json`,
  `templates/expo-template-default/app.json`
- package.json commit: `d01580058a40e2650efca4dad1015255d500bf99` (committed 2026-06-17)
- app.json commit: `34c94f43b89c73f06d5a1b1a74f2438f993b91d4` (committed 2026-01-30)
- Raw URLs:
  - https://raw.githubusercontent.com/expo/expo/d01580058a40e2650efca4dad1015255d500bf99/templates/expo-template-default/package.json
  - https://raw.githubusercontent.com/expo/expo/34c94f43b89c73f06d5a1b1a74f2438f993b91d4/templates/expo-template-default/app.json
- License: package.json declares `"license": "0BSD"`; the expo/expo repo
  license is MIT. Both permissive.
- Detector-relevant keys: `dependencies.expo`, `dependencies.react-native`,
  `dependencies.react` (+ app.json `expo` key) → expected platform `mobile`,
  `mobile.expo` true, `mobile.reactNative` true.

## empty/.gitkeep

- Not upstream content: an empty placeholder so git tracks an otherwise-empty
  project directory. Represents an undetectable project → expected platform
  `unknown`, exit 0.
