# Access Kit 0.1 verification

2026-11-27, Rust1.100.0; exact protocol/security crate versions recorded in Cargo.toml. Cargo metadata reports rmcp/openidconnect MSRV1.88; production hosts must build with a supported toolchain and retain their root lockfiles.

`cargo test --all-features`: 27 passed (security10, OAuth5, MCP4, provider2, review regressions6). Provider tests include multiple negative signed-JWT cases inside the actual HTTP flow. These are behavior/security tests, not counts inferred from source.

- Literal canonical origins; empty deployment env fallback; disallowed remote HTTP/userinfo/path/query/fragment.
- Real cookie key creation, raw-once/hash-only listing, api/mcp audiences, missing scopes, expired/revoked/disabled authority, cookie/bearer separation, CSRF and current host permission denial.
- Native form consent, registered exact redirect, no redirect for an unvalidated client URI, S256wrong verifier/code expiry, code single-use/concurrent redemption, refresh scope/resource restriction, refresh rotation/reuse family revoke, foreign-client revocation isolation.
- Actual official rmcp3.12 SDK2026 Discover/list/call, per-request authorization after revocation, unsupported legacy/missing bearer/Host rebinding denial; protocol activity excluded from tool counts. Two real SDK servers with identical tool names and different schemas remain isolated.
- Authenticated malformed RESTJSON and tools/call rejected before SDK handler both count one failed attempt. Private payload/token/unknown operation strings do not enter safe audit events. Unauthorized events admin-only; self/admin audit filters enforced.
- Reserve failure never calls domain host; finalize failure returns503 and keeps pending; interrupted/cancelled request restart recovery changes outcome and aggregate once; UTC zero-fill/day count correct.
- Actual local GitHub OAuth token/user numeric subject; actual local Google discovery/JWKS/token HTTP with deterministic public fixture RSA signatures; bad issuer/audience/nonce/signature/expiry rejected. No email merge/open registration. Unbound identity explains original login/link. Different browser/session and replay rejected; identity conflict handled; bound login issues original host session and updates real last_login_at. Used verifier/nonce material cleared.
- Duplicate/write/remote-schema registries rejected at startup. JSONschema network/file resolver features disabled.

`cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` pass. `python scripts/test_snapshot.py`:5 passed for deterministic hash/tamper detection and file/directory symlink rejection. `python scripts/snapshot.py --check` verifies exact source/schema bytes.

This is core evidence. Each host still must run its own real operation success+permission denial, frontend lifecycle, database migration and Docker build; SDK log hard-filter and untraced internal domain router are host requirements. Production GitHub/Google app registration and live callback validation were not run without deployment credentials; mock results are labelled accordingly. No GPU/model execution is claimed here.

## Independent review corrections

Four Important findings were reproduced locally, then covered by regression tests: invalid OAuth requests consuming trusted rate capacity; permanent anonymous registration quota blocking administrators; deferred SQLite audit transaction upgrade contention under30 simultaneous WAL requests; oversized MCP responses recorded as successes. Schema2 separates public registration lifecycle, default-off discovery and managed registration. Additional tests cover known-client invalid requests, default metadata,24hour unused-client expiry, version1→2 migration preservation, and destination/ancestor symlink rejection before snapshot replacement. Final test totals and fix review are recorded in the host PRs.


## Schema 3 registration verification — 2026-11-27

The preceding schema2 evidence remains the historical baseline. Schema3 adds account policy, applications and a separate registration audit. The current all-features core suite has32 tests, including5 registration tests. New coverage exercises default-closed policy and administrator-only mutation, authenticated signup rejection, explicit Origin enforcement on new registration mutations, a concurrent500-account cap, final-provider unlink protection, Google signup callback policy recheck, pending/rejected verified-identity login, existing-identity signup conflict, GitHub open signup, transactional administrator revalidation, and refusal to approve a corrupted pending administrator. Mock provider HTTP still verifies real OAuth/OIDC exchanges; no production SSO success is claimed.

Host integration tests additionally exercise legacy account ID42 migration with active/password-enabled defaults, setup, approval/open/closed password registration, review races and stale409, rejection reasons, password/cookie/domain activation gates, disabled-toggle isolation, origin rejection, unknown-field/role injection, rate limiting, pagination and redacted registration events. New host migrations append the byte-identical core0003 registration SQL and account flags; old migration bytes stay unchanged. KEYPO signup also initializes the same five suggested presets as administrator-created accounts inside the registration transaction; injected audit failure rolls account and presets back together. Hosts must run their own test/build/Clippy and browser checks; core32 tests do not replace host validation.

Current core `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, snapshot integrity checks and all-features tests are required for the schema3 snapshot. Source and schema hashes in SNAPSHOT.json identify the exact revision.
