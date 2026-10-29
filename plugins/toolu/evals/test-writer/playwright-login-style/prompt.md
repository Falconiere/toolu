---
description: Playwright spec style — role/label locators, web-first assertions, no fixed waits, no CSS selectors.
tags: [behavior, playwright]
max_turns: 10
timeout_seconds: 300
allowed_tools: [Read, Glob, Grep, Skill, TodoWrite]
---

Write a Playwright Test spec at `tests/login.spec.js` for our login page. Do not run it;
Playwright is not installed here.

- The page at `/login` has a form with the labels "Email" and "Password" and a "Sign in"
  button.
- A correct login redirects to `/dashboard`, which shows the heading "Welcome, Ada".
- A wrong password shows an alert with the text "Invalid email or password".
