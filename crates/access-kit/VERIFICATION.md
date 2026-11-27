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
