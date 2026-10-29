---
description: Appium spec style — accessibility-id locators, no XPath, no fixed sleeps.
tags: [behavior, appium]
max_turns: 10
timeout_seconds: 300
allowed_tools: [Read, Glob, Grep, Skill, TodoWrite]
---

Write an Appium end-to-end test with WebdriverIO and Mocha at `test/login.e2e.js` for our
mobile app's login screen. Do not run it; no device is attached.

- The screen has an email field, a password field and a "Sign in" button.
- After signing in, the home screen shows the text "Welcome".
- The app team will add accessibility identifiers if the test needs them.
