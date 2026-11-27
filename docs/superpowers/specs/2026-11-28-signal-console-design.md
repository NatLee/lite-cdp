# Signal Console: explicit identity and inspectable collection

Status: implementation specification under the user's delegated product decisions. Parent design: `../../../../design-notes/2026-11-28-product-evolution-design.md`, section 3.

## Contracts

- Existing sites and API-created sites without `collection_policy` retain the legacy collection profile. The UI offers a recommended profile: consent required, pageviews on, automatic clicks/scroll off, query stripping, origin-only referrers, no click text, explicit trait allowlist. Saving is explicit. Existing auth, public API/MCP operation names/scopes, nginx and access-kit are unchanged.
- Public `GET /tracker-config/{key}` returns versioned effective collection policy, active state and stable site storage namespace. It contains no signing secrets. Tracker remains `/t.js`; one script per document; identical loads reuse the singleton, conflicting key/endpoint raises a diagnostic event and warning.
- Browser identity is random, first-party and site-scoped. A single legacy storage namespace may be claimed by only the first migrated site; localStorage identity is not a fingerprint. A shared reset/revocation storage signal rotates IDs across open tabs. Session lifetime remains per-tab, not a 30-minute analytics session.
- `identify.user_id` is the preferred site-scoped stable subject. Email-only clients retain legacy matching. Distinct nonempty user IDs never merge by email. Legacy email profiles upgrade only when the current browser's matching email explicitly accompanies the first user ID. Public identify is a claimed identity, not authentication. Optional HMAC user-ID assertions provide server-verified subject identity without exposing the signing secret to browser code.
- Policy: consent_required, respect_dnt, auto_page, spa_mode (history/hash/manual), auto_click, auto_scroll, query_mode (strip/allowlist/keep), query_allowlist, referrer_scope (none/origin/full), click_text, ignore_selectors, allowed_traits (null means legacy all), legacy_email, require_signed_identity. Bounded fields are validated. Client sanitizes before network; collector enforces again. DNT or pending/denied consent means no IDs, event queue, or collection requests. Config lookup is not a collection event.
- Consent grant creates IDs and starts listeners. Revoke drops queued/retry events, stops collection, clears IDs and broadcasts revocation. Consent state is not silently remembered by tracker; CMP replays choice on each document. Reset/logout invalidates old in-flight identity in sibling tabs before their next event; previously sent requests cannot be recalled.
- New events have site-unique event_id and happened_at. Received time is stored separately. Timestamp window: 7 days past / 5 minutes future. Legacy events without ID/time remain accepted and retain prior behavior. Retry same IDs only, bounded to 3 attempts and 200 queued events; terminal errors are visible. No exactly-once network claim.
- Installation checks use admin-created random expiring test tokens. Collector validates origin, policy and payload then stores bounded receipt only; no person/event/label rows. Authenticated site-visible receipt route returns progress. Tokens expire after 15 minutes and are site-bound.

## Product

Install workbench has framework tabs (HTML, React/Vite, Next App Router, Vue/Vite, Nuxt 3/4, GTM), shared consent/login/logout recipes, effective policy editor, synthetic identity simulator, and real receipt verification. Templates explain StrictMode/SSR and avoid router-hook double pageviews. Simulator never sends network requests; shows sanitized inputs, payload, deterministic matching and journey. Consent/duplicate/conflict/clear-storage/logout scenarios are keyboard operable.

Signal Console uses graphite #101211, chalk #F4F5EF and olive #596D00 / #C7E45B. Neutral surfaces stay neutral, borders are visible, labels and values have clear hierarchy. Two font weights, 4px rhythm and reduced motion. Editable SVG cover shows anonymous streams joining only at explicit identify. README captures come from the real app and disposable fixtures, not invented customer data.

## Verification

Regression tests cover legacy API behavior, stable-ID isolation/same-ID merge, email conflicts, policy enforcement, timestamp/idempotency, test receipt isolation/access/expiry, tracker consent/revoke/DNT/duplicate/conflicting load/storage/retry/SPA. Frontend tests cover policy persistence, templates and simulator. Run complete Rust/frontend tests, formatting/build, real Chrome tracking and theme/mobile flows, then capture evidence. Limit Cargo jobs to 2, debug info to 0, Vitest workers to 2. No paid inference, outbound messaging, push, PR or archive in this workstream.
