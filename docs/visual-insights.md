# Visual insights

An engagement atlas: current identity × last-seen cohorts, linked to matching people/CSV filters, plus a keyboard-accessible daily traffic explorer. Cohorts describe current state, not conversion or journey transitions; daily unique visitors are never summed as period uniques.

Use existing React, React Router, CSS and SVG; inspected the existing query, hooks and routing APIs. No chart dependency is needed. Keep useful existing pages and administration workflows.

- [x] Add matching API aggregates and filters with tenant isolation.
- [x] Test filter boundaries, invalid values, counts and CSV consistency.
- [x] Build responsive interactive visual components, empty/loading/error states.
- [x] Verify full Rust/frontend checks and actual Chrome desktop/mobile/dark workflows.
- [x] Refresh README, editable cover and screenshots.

## Interaction and accessibility

All drilldowns are ordinary links with shareable query parameters. Chart values are available as text and accessible names, not color alone. Focus indicators are visible; mobile layouts retain the controls; reduced-motion preferences disable decorative transitions. Empty and failed requests explain the next action and provide a retry button. Existing setup, login, theme, administration and data workflows remain available.

## Validation

- `cargo fmt --check`, `CARGO_BUILD_JOBS=2 cargo clippy --all-targets -- -D warnings`, `cargo test`: 25 unit + 18 integration tests passed.
- `pnpm test`: 17 tests passed; `pnpm build` passed.
- The new API integration test first failed on the missing `engagement` aggregate, then passed with all six cohort/list/CSV counts and site access checks. A fixed-clock SQL test covers both sides of the 7-day and 30-day boundaries.
- Extreme pagination (`page=i64::MAX`) reproduced a multiplication-overflow panic; a saturating offset now returns an empty page and preserves the total. The regression was verified failing with the old multiplication and passing with the fix.
- `docs/smoke-test.sh` passed against a fresh temporary database and the native binary.
- `docs/verify-insights.py` passed in actual Chrome: all six cohort drilldowns/CSV counts; metric switching and keyboard date slider; site switching; 1440px and 390px in light/dark with no page errors or horizontal overflow.
- `docs/capture-screenshots.py` refreshed all README images from an isolated seeded demo. Visual review caught and fixed SVG rules applying to nested navigation icons; the browser script now checks those icon widths.

## Data definitions

Current `email != ''` means identified member. Activity uses the current `last_seen`: recent ≤ 7 days, cooling > 7 and ≤ 30 days, dormant > 30 days. These groups are exhaustive and mutually exclusive, subject to the site's retained data; aggregate and list/export share the same SQL predicate. Percentages use all people at the current site. Bubble area scales with cohort size (the radius is proportional to the square root); outline guides are decorative. This is a current-state map, not a conversion funnel or historical transition model. The seeded demo has no dormant people, so those cells correctly show zero.

Daily pageviews count pageview events. Daily visitors are unique within a local calendar day and are never summed into period uniques. The chart uses one selected metric and one labeled scale at a time; its date selector also shows the other metric for that day. Counts can change as live time advances or data is retained/deleted.

## Reproduction and scope

Use a new data directory with the existing `demo` command; never seed or reset a production database. With Playwright and Chrome installed, run `DEMO_URL=... DEMO_PASSWORD=... python docs/verify-insights.py`, then the capture script with the same environment. A synthetic demo account is required; credentials are supplied through environment variables and are not committed.

No new packages, schema migration, external analytics, delivery integration or network service was added. This iteration verified native application workflows, not Docker image rebuilding or external SMTP/webhook delivery. Hosted CI was unavailable during the preceding iteration because of an account billing/spending-limit gate; local evidence is recorded here rather than claiming hosted checks passed.
