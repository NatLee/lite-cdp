# Registration and review verification

Verified 2026-11-27 against local Google Chrome 158.0.8127.37, the built React frontend, and the Rust host binary. Every browser run used a new disposable SQLite database. No production accounts or data were modified. Owned servers, browser contexts, and temporary databases were removed after each run.

## Automated checks

- Full frontend suite: `cd src/web && pnpm exec vitest run --maxWorkers=2`: **60 tests passed**.
- `cd src/web && pnpm build`: passed. Keypo retains its existing bundle size advisory.
- Source spacing/weight/effect audit: passed using the existing `keyword-vault/docs/audit-styles.py` rules with this repository as ROOT.
- Twelve added frontend cases cover closed registration, confirmation mismatch, pending isolation, configured providers only, safe OAuth continuation, first-admin priority, active session refresh, policy/rejection controls, queue/audit pagination, stale 409 refresh, applicant account badges, and saving a 100-character profile name without truncation.
- All six suites initially run together with default workers saturated the host and produced old and new test timeouts. Complete suites subsequently passed in two-repository batches with two workers per repository; timeout limits were unchanged.

## Reproduce the browser checks

Install Google Chrome and Python Playwright, build both application layers, then run the repository-local harness:

```sh
cargo build --manifest-path src/api/Cargo.toml
(cd src/web && pnpm install --frozen-lockfile && pnpm build)
python3 -m venv .venv-browser
.venv-browser/bin/pip install playwright
.venv-browser/bin/python docs/verify-registration.py
```

The default binary is under `src/api/target/debug/`. If using `CARGO_TARGET_DIR` or another build directory, pass `--binary /absolute/path/to/the/binary`. The harness chooses a free local port, creates a fresh temporary database, exercises the full lifecycle and 64/256/100 boundaries, captures eight screenshots, prints its check results, and stops/deletes everything it created. `--output-dir /path/to/images` selects another screenshot directory. It never connects to an existing server. These checks require only local password registration; no provider credentials are required.

## Real Chrome behavior

The UI was exercised with real HTTP requests and SQLite persistence:

1. `/register` on an empty installation redirected to first-admin setup. Creating the administrator entered the workspace.
2. Anonymous `/register` defaulted closed and exposed no password/provider signup actions.
3. The administrator switched to approval mode. A short password and mismatched confirmation each displayed validation errors and sent **zero** registration requests.
4. A valid application remained outside the workspace. The anonymous browser had **no cookies**, `/api/auth/state` returned `user: null`, and `/api/access/overview` denied access.
5. The administrator reviewed and approved the application in the UI; password login then succeeded. The queued safe continuation reached the local `/oauth/authorize` endpoint. This check used a synthetic client and did not claim a completed OAuth grant.
6. A second application could not be rejected without a reason. After entering a reason and rejecting, login was denied and auth state remained anonymous. The rejected filter displayed the stored reason.
7. Open mode created an active session. An external continuation URL was discarded, and the browser remained on this origin. Registration and policy events appeared with UTC timestamps and actors.
8. A separate final-binary boundary check created an account with a **64-character username, 256-character password, and 100-character display name**, edited that profile name to another 100-character value, logged out, and logged back in successfully.
9. Desktop and mobile registration inputs remained at least 220px wide; policy selectors remained at least 150px wide. All captured viewports had no document horizontal overflow, and the completed browser flows had no JavaScript page errors.

The screenshots contain blank password fields. All credentials were temporary and are absent from screenshots and this document.

## Screenshots

Registration was captured at 1440px and 390px width in both themes. Review screenshots focus on the registration panel, retaining the selected viewport width so controls and audit entries remain readable.

| Surface | Desktop light | Desktop dark | Mobile light | Mobile dark |
| --- | --- | --- | --- | --- |
| Registration | [View](images/registration-register-desktop-light.png) | [View](images/registration-register-desktop-dark.png) | [View](images/registration-register-mobile-light.png) | [View](images/registration-register-mobile-dark.png) |
| Policy and review | [View](images/registration-review-desktop-light.png) | [View](images/registration-review-desktop-dark.png) | [View](images/registration-review-mobile-light.png) | [View](images/registration-review-mobile-dark.png) |

## Limits of this evidence

Google/GitHub provider credentials were not configured in these disposable installs, so no live provider consent or SSO success is claimed. Frontend tests verify configured-provider signup links and explicit registration intent; provider transport and token lifecycle coverage belongs to the backend suite. This document records local browser/frontend checks; overall backend, Docker, independent review, and CI results are recorded separately.
