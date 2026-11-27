# Signal Console verification — 2026-11-28

The delivered slice replaces the blue visual system with graphite/chalk/olive, adds an editable cover and installation workbench, and implements explicit site-scoped identity, collection policies, retry metadata and isolated receipts. Historical email-only clients and the frozen public API/MCP surface remain covered.

## Executed checks

| Check | Result |
|---|---|
| Complete Rust suite, two test threads | 65 passed: 29 unit, 25 API, 6 access/MCP, 4 registration, 1 collection migration |
| Actual legacy-database upgrade | Existing email identity, device linkage, event and counters preserved; legacy email uniqueness retained; stable subjects can share email |
| Tracker VM lifecycle/transport suite | 12 passed: consent/DNT, grant/revoke, pre-config denial, cross-tab reset, same/conflicting load, sanitize-before-send, ready initialization and legacy claim, history/hash/manual, bounded retry with same IDs, non-serializable payload, oversized batch splitting |
| Frontend | 14 files / 64 tests passed, including policy save, framework template and synthetic merge/re-consent behavior |
| Production frontend | TypeScript and Vite build passed |
| Formatting/source checks | cargo fmt, git diff --check, 37-file frontend spacing/weight/icon/surface audit passed |
| Real Chrome + native Rust/SQLite | Full tracker and UI flow passed; no page errors; computed styles and no horizontal overflow at 1440 and 390 px |
| Docker Compose build | Both production targets built with Cargo jobs limited to 2 |
| Real nginx/Compose feature checks | Config JSON/CORS/no-store/no-secret, tracker MIME, consent 403, event replay once, URL strip, receipt without analytics, nonroot/internal API passed |
| Existing nginx compatibility check | Chrome 158.0.8127.37, max-length signup/profile/logout/login; registration modes; REST/MCP discovery/list/call/revoke; backend recreation retaining session and stable nginx all passed |

The retry browser check forwards the first request to the real collector, deliberately replaces its response with 503, then forwards the retry. Two delivery attempts produce one pageview. Two browser contexts identify the same `user_id` and produce one member with two devices. A different user ID sharing that email remains a separate member. Consent denial creates neither tracking IDs nor collection traffic; logout/revoke propagate to another tab.

Tests use disposable SQLite databases, a local fictitious shop, generated test credentials and isolated Compose projects. No customer data, production volume, OAuth provider, paid service or LLM was involved. Both verification Compose projects were removed with their own disposable volumes after the checks.

## Evidence and reproduction

- [Browser result](signal-console-browser.json), [Compose result](signal-console-compose.json).
- [Real Chrome verifier](verify-signal-console.py): starts a native Rust server, seeds isolated demo data, starts a separate-origin shop, runs identity/retry/receipt/UI scenarios, and writes current screenshots.
- [Compose verifier](verify-signal-compose.py): consumes built images, starts a fresh project bound to a random loopback port, validates the proxy and collector, then removes only that project's containers/volume.
- [Editable SVG cover](images/cover.svg), [cover preview](images/cover-preview.png), [identity lab](images/signal-identity-light.png), [dark lab](images/signal-identity-dark.png), [mobile lab](images/signal-identity-mobile.png), [framework teaching](images/signal-install-light.png).
- README overview/people/person/labels/segment/install/journey/mobile screenshots were refreshed from the same current build. Old `workflow-*` images and their older verification documents remain historical artifacts; README now features Signal Console captures.

```sh
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --manifest-path src/api/Cargo.toml -- --test-threads=2
cargo fmt --manifest-path src/api/Cargo.toml --check
node --test src/api/tests/tracker.test.mjs
pnpm --dir src/web exec vitest run --maxWorkers=2
pnpm --dir src/web build
python3 docs/frontend_style_audit.py
# Python with Playwright installed and Google Chrome available:
python3 docs/verify-signal-console.py
# Separate production images; no customer volume is opened by the verifier:
docker compose build --build-arg CARGO_BUILD_JOBS=2
python3 docs/verify-signal-compose.py
```

Native Chrome verification on this workstation used `/Users/natlee/miniconda3/bin/python3`. The existing broader nginx regression script was `/tmp/verify-nginx-compose.py --skip-build lite-cdp`; its result and cleanup logs are `/tmp/lite-cdp-nginx-compose.result.json` and `/tmp/lite-cdp-nginx-compose.cleanup.log`. This auxiliary cross-project script is not a repository runtime dependency.

## Boundaries

The browser test executes the actual HTML tracker and generated Next snippet content, not separate compiled applications for every framework. Framework placement/lifecycle guidance was checked against official React, Next, Vue Router, Nuxt 3/4 and Google consent documentation linked in [tracking.md](tracking.md). Production CSP/CMP integration, third-party blockers and framework-specific routing should be checked in the host application.

Identity is deterministic and site-scoped. Unsigned IDs are claims; optional HMAC proves only the member ID, not email/traits or event truth. No probabilistic fingerprinting, automatic cross-subdomain IDs, persistent offline queue, exactly-once delivery, signature rotation UI, or consent legal-record system is claimed. Event deduplication is bounded by retained event rows. Historical data survives revocation until deletion/retention acts; consent choices must be supplied by the website's CMP on each document.

Specs and plan live in `docs/superpowers`. This record covers local implementation and verification; it does not claim a remote push, PR, merge, deployment or archive.
