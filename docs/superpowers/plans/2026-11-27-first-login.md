# lite-cdp First-login Implementation Plan

> For agentic workers: use executing-plans within the assigned repository; root coordinates independent domains with dispatching-parallel-agents.

**Goal:** Provide a themed first-entry account creation page and transition into the existing app.
**Architecture:** Dedicated /first-login route, existing account/session APIs, guarded initial administrator creation.
**Tech Stack:** Existing project UI/backend; no new dependencies required.
**Spec:** ../specs/2026-11-27-first-login-design.md

## Global Constraints
Retain existing roles, OAuth scopes and domain permissions. Keep frozen crates/access-kit unchanged. Use400/600 weights and4px spacing. No raw passwords or setup tokens in URLs, logs, persistent browser storage or screenshots. Prior direct execution/PR merge authorization persists.

## Review Focus
- Two simultaneous first visitors must create only one administrator.
- Existing account state must close setup, including a409 from another browser.
- Untrusted next parameters cannot redirect off-origin.
- Password mismatch and duplicate submissions must not create requests/accounts.
- Docker/remote setup must not silently drop loopback protection on ES/Sentiment.

### Task 1: Entry and account form
**Files:** src/web/src/App.tsx, context.tsx, pages/Login.tsx and a focused pages/FirstLogin.tsx plus existing theme CSS; corresponding Login/FirstLogin tests.
- [ ] Add meaningful failing cases for /first-login routing, matching passwords, confirmation mismatch with zero POSTs, conflict refresh, success and safe continuation.
- [ ] Run targeted tests and observe expected RED.
- [ ] Implement dedicated first-login UI, lifecycle routing and context refresh using existing setup/login contracts.
- [ ] Run targeted and full frontend tests plus production build; expect all PASS.

### Task 2: Backend setup compatibility
**Files:** Existing src/api auth/setup is retained; review its atomic guard without altering shared access-kit.
- [ ] Verify atomic first-account semantics. For ES/Sentiment add failing token-protected nonloopback cases before implementation; preserve native loopback and CLI behavior.
- [ ] Implement only required host changes; do not change vendored access-kit.
- [ ] Run affected full backend tests, fmt/lint and production compilation; expect all PASS.

### Task 3: Browser, docs and delivery
**Files:** README.md; docs/first-login-verification.md; docs/images/first-login-*.png.
- [ ] Run actual browser against empty disposable DB: mismatch→create→authenticated→logout→login; existing /first-login closes; inspect1440/390 light/dark and reduced motion.
- [ ] Capture first-login screenshots with password/setup-token fields empty. Update canonical setup instructions including deployment token only where required.
- [ ] Commit explicit changes; report evidence. Root independently reviews, pushes PR, verifies CI or documents existing infrastructure block, merges exact reviewed head and deletes branch.
