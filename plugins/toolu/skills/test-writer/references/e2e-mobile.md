# Mobile E2E: Appium, Detox, Maestro

## What runs on a device

Component behavior is tested with React Native Testing Library under Jest (`typescript-react.md`). Use a device only for a few critical flows: login, purchase, deep links, permissions, and background or relaunch.

## Pick the tool

- **Detox:** React Native, gray-box. It waits for the app to go idle, so it stalls on endless animations or polling.
- **Maestro:** black-box YAML flows with built-in retries. Least setup.
- **Appium:** any app, any language, real-device clouds. Waits are your job.

Use the tool the repository already has. Published speed comparisons between them conflict, so measure on your own app.

## Write it well (Appium)

- Locators: accessibility ID, then resource ID, then iOS predicate or class chain, and XPath last. If you must use XPath, anchor it on attributes, never on an absolute or index-based path. A missing accessibility ID is an app bug: add it to the app.
- After the app is backgrounded or relaunched, wait for a known element before acting.
- Re-query elements after navigation; old references go stale.
- Open the screen under test with a deep link instead of tapping through setup screens.
- Limit deep iOS view trees with the `snapshotMaxDepth` setting.
- No coordinates and no fixed sleeps.

## Run it cheap

- Reuse one session for the whole suite, and restart the app between tests (`terminateApp`, then `activateApp`). Use `noReset: false` or `fullReset: true` only for install or onboarding tests.
- Run at most one emulator or simulator per agent. Boot it once and keep it running.
- Take screenshots only on failure.
