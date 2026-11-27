# Access Kit 0.1

Shared Axum0.8/SQLx0.8 SQLite access core. Hosts own users, password login, cookie sessions and all resource authorization. See [INTERFACES.md](INTERFACES.md) before implementing an adapter.

The core serves real personal credentials, GitHub/Google account binding/login, public-client Authorization Code+S256PKCE with rotating refresh tokens, RFC7009 revoke, protected-resource/authorization-server discovery, the fixed domain REST registry, and official rmcp3.12.0 StreamableHTTP2026-07-28. Legacy MCP sessions/initialize are unsupported. Unknown/unbound identities cannot create accounts or merge by email.

Apply the byte-identical `migrations/0001_access.sql` and `migrations/0002_client_registration.sql` in order through the host's next numbered migration before `AccessKit::new`. Do not rerun it independently or change the host's0001 migration. Store the Clone+Send+Sync kit on host state, using Weak in the adapter to avoid cycles. Merge `kit.router()` separately from legacy cookie-only origin middleware. Mount an untraced in-process domain router for fixed Host::execute mappings; never proxy arbitrary URLs.

`AccessConfig::from_env(literal_loopback_origin)` accepts ACCESS_PUBLIC_URL (unset/blank falls back), ACCESS_GITHUB_CLIENT_ID/SECRET, ACCESS_GOOGLE_CLIENT_ID/SECRET. Remote origins must use HTTPS. Fixed provider endpoints are used in production. Configure original host public URL/cookie Secure behavior from the same canonical origin; conflicting origins must be rejected. Callback URIs are `{origin}/oauth/callback/github` and `/google`. Missing credentials leave a disabled provider with an explanation. Actual production provider registration/callback verification is a deployment step; local mock success is not production login evidence.

## Audit and logging

API operations and recognized MCP tools/call requests reserve a durable pending attempt before domain execution, including malformed tools/call requests rejected by the SDK. Domain dispatcher reuses the reserved ticket. Finalization is conditional, so a call is counted once. Discovery/list/protocol activity never increments domain counts. Reserve failure denies execution; finalize failure503 leaves pending. Recovery marks only bounded stale pending attempts interrupted/failure after600seconds. Each event/daily update pair is transactional; no write transaction spans host execution or external HTTP. UTC detail retention90days; durable aggregates back7/30/90day views with pending separate.

The library never logs payloads, tokens, cookies, codes, provider responses or private query values. **Hosts must also hard-filter all tracing targets beginning `rmcp` at the formatting layer, even with RUST_LOG=trace**, because the upstream SDK can emit full request/results at debug/trace and arbitrary notifications at info. An EnvFilter directive alone can be overridden by more-specific target directives. Never wrap the internal domain oneshot router or access/OAuth endpoints in URI tracing that exposes arguments/callback codes. Use the typed metadata audit instead.

Caller-supplied request IDs and trusted-principal headers are ignored. Per-request TrustedPrincipal extensions have private constructors and cannot be deserialized; the host extractor re-reads active full user/role before checking resource permissions. Personal credentials have explicit api/mcp audiences; OAuth tokens have one exact resource. Access expiry is capped by the grant's original30day deadline, also checked on every bearer request.

On explicit host account deletion, remove that user's external identity bindings and revoke credentials/grants in the same deletion transaction; retain audit. Disabling an account retains its bindings and rejects all token use via active_principal.

## Testing and mock provider

```sh
cargo test --all-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

The suite uses real localhost SDK and provider HTTP, with deterministic public test RSA material. It does not contact production providers. For native host browser verification:

```sh
cargo run --features test-support --example mock_provider -- 127.0.0.1:19090
```

Build the host with `--features access-kit/test-support`, then set `ACCESS_TEST_MOCK_PROVIDER_URL=http://127.0.0.1:19090` and its canonical ACCESS_PUBLIC_URL. The feature-only config accepts loopback exclusively and enables mock-client/mock-only-secret solely for that mock issuer. Production builds neither read this env nor support endpoint overrides. The mock is an intentionally limited demonstration issuer, never an identity provider for production accounts. For browser binding start from an existing host account's Connections page; it will run real state/PKCE/nonce/code exchange and produce a persisted identity. Google JWT issuer/audience/expiry/nonce/signature are verified by openidconnect4.1.0.

New host servers should use `into_make_service_with_connect_info::<SocketAddr>()` to supply trustworthy socket peer identity for DCR limits; when absent, a conservative shared fallback bucket is used. Forwarded headers are not trusted for rate identity.

## Immutable snapshots

`python scripts/snapshot.py --write` records exact source/schema hashes. `--check` validates a snapshot; `--destination /absolute/host/crates/access-kit` copies those same bytes and verifies them. Cargo.lock/target/.git are authoring/build artifacts, excluded from source identity; each host keeps its root lockfile. Snapshot includes tests/example so the standalone contract can be reproduced. Hosts must not edit vendored protocol files independently; amend authoring, retest, regenerate and replace all seven snapshots together.

Public client registration is disabled by default. Set `ACCESS_DYNAMIC_CLIENT_REGISTRATION=true` to advertise and enable `/oauth/register`; administrator registration remains available. Public registrations without a user grant expire after24hours, unless a live consent/code still references them. Their pending quota never consumes administrator registration capacity. Schema2 is additive: host migrations must preserve original0002 bytes and add the new core migration as0003. Existing clients remain managed.

Rate identity uses the trusted socket peer for unauthenticated token attempts. Invalid client IDs/codes cannot consume a verified user's token budget. MCP finalization occurs after collecting the SDK response, including transport/body-limit failures; SQLite audit writes reserve the writer with `BEGIN IMMEDIATE` before reading so concurrent WAL requests do not strand successful calls.
