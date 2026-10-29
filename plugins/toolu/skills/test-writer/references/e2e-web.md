# Browser E2E with Playwright

## What to cover

Only the critical user journeys: sign-in, checkout, the main create and edit flows. Edge cases belong in component or integration tests.

## Write it well

- Locate elements with `page.getByRole('button', { name: 'Submit' })`, `getByLabel` and `getByText`. Do not chain CSS or XPath selectors. Use `getByTestId` only as a last resort.
- Use web-first assertions, `await expect(locator).toBeVisible()`, rather than `expect(await locator.isVisible()).toBe(true)`.
- Never call `page.waitForTimeout`. Rely on auto-waiting and assertions.
- Isolate every test: its own context and storage, and data that the test creates itself (through an API call in a fixture). Never depend on data another test left behind.
- Stub third parties (payments, analytics, external links) with `page.route`, so tests cover only what you control.
- Log in once: a `setup` project saves `storageState`, and the other projects depend on it. If tests change server-side state, give each worker its own account.

## Run it cheap

- Edit loop: `npx playwright test <spec file> -g "<title>" --project=chromium`.
- Set `use: { trace: 'on-first-retry', video: 'on-first-retry', screenshot: 'only-on-failure' }`.
- Turn on `fullyParallel: true` only once tests are isolated. Set `workers` explicitly when agents share the machine.
- Reuse a running dev server (`webServer.reuseExistingServer`) instead of starting one on every run.
