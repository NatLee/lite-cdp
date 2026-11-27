# nginx Compose deployment

`docker compose up -d --build` starts nginx and the internal `lite-cdp` API. Open **http://127.0.0.1:8585**. Only nginx publishes `${CDP_PORT:-8585}`; the API keeps port `8585` on the Compose network and runs as UID 10001. The existing `lite-cdp-data` volume remains mounted at `/app/data`.

Copy `.env.example` to `.env` for overrides. `CDP_PORT` changes both the public port and the default canonical URL. Set `ACCESS_PUBLIC_URL` to the exact browser origin, including a nonstandard port, when using another hostname or external HTTPS. The legacy `CDP_PUBLIC_URL` remains supported. Either URL supplies the other when unset; explicitly configured values must have matching origins. Provider callbacks stay `/oauth/callback/github` and `/oauth/callback/google` on that same origin. External HTTPS termination is configured outside this Compose file; nginx does not trust caller-supplied forwarding headers.

nginx serves the built React assets, returns the SPA for client routes such as `/register`, and returns 404 for missing `/assets/` files. It proxies `/api`, `/oauth`, `/.well-known` and `/mcp` without stripping prefixes or rewriting queries. Host (including port), Origin, Authorization and Cookie are preserved. Forwarding headers are replaced with nginx's socket peer and scheme. HTTP/1.1, disabled request/response buffering and proxy timeouts of 300 seconds support MCP streaming. The global body limit is 16 MiB; application-specific limits still apply. Access logs record `$uri`, never OAuth query strings.

Check the proxy and application separately:

```sh
docker compose config
docker compose up -d --build
curl --fail http://127.0.0.1:8585/healthz
curl --fail http://127.0.0.1:8585/api/health
curl --fail http://127.0.0.1:8585/api/auth/registration-policy
docker compose ps
docker compose logs --tail=50 nginx
```

`/healthz` checks nginx itself; `/api/health` reaches the real backend. Preserve the named data volume during upgrades: `docker compose down` stops services, while `docker compose down -v` deletes the database and is only appropriate for disposable test deployments.

The Dockerfile has three runnable targets: `runtime` (API only), `frontend` (nginx + built React), and final/default `standalone` (the original nonroot API + static files). Existing direct builds remain supported:

```sh
docker build -t lite-cdp:standalone .
```

Compose builds the two smaller deployment targets explicitly. The backend remains reachable to other services on the Compose network, rather than through a published host port. Application authorization and registration policy remain enforced by the backend.

`/collect` and `/t.js` also proxy to the API. Compose defaults `CDP_TRUST_PROXY=true` because nginx overwrites X-Forwarded-For; collection rate limiting can then use the visitor address safely. Existing tracking-site CORS behavior stays in the application.

Proxy semantics follow the [nginx proxy module](https://nginx.org/en/docs/http/ngx_http_proxy_module.html#proxy_pass) and the path-only access format uses the [nginx log module](https://nginx.org/en/docs/http/ngx_http_log_module.html#log_format).


Static success and error responses include DENY, nosniff and same-origin security headers. Each static location includes the full header set so nginx inheritance does not drop a header; proxied responses retain the backend headers without duplicate additions. Missing images, favicons and other static file requests return 404 instead of SPA HTML. Docker DNS re-resolves the backend every 5 seconds, allowing a replaced backend container to recover without restarting nginx.

Server-scoped nginx error logging is disabled because request-bearing errors can include OAuth code/state query strings even at critical severity. Main-process startup/configuration errors still use the nginx image's stderr logger. The safe access log retains method, path, status, upstream status and connection/response times to diagnose backend failures without logging queries or credentials.

## Actual Compose verification,2026-11-28

`docker compose build` built the final API and nginx targets. API image: `sha256:1b8d7593ed472296de1f5b20076aac9b4b883028fac74251acfca6069438c38a`; nginx image: `sha256:aa4ad52c01d9325333f60495e5bdd492ec51726b7dd4585f55b61e745a7c93ed`. Build log: `/tmp/lite-cdp-nginx-compose-build.log`.

`/Users/natlee/miniconda3/bin/python3.12 /tmp/verify-nginx-compose.py --skip-build lite-cdp` passed against a fresh, isolated Compose project at `http://127.0.0.1:28585` in4.9seconds. It checked nginx/backend health, static security headers and missing-file404, closed/approval/rejected/open registration, pending denial, approval, stale review409 and member policy denial. Bearer REST plus MCP2026-07-28 discover/list/call ran the real `list_sites` operation; revoking the key immediately produced401. `/t.js` retained its JavaScript response, and malformed `/collect` requests reached the backend rather than SPA HTML.

Installed Chrome158.0.8127.37 exercised the built React app through nginx with a64-character username,256-character password and100-character display name, then edited the profile, signed out and signed back in. The API ran nonroot with no published host port. Recreating only the backend preserved the administrator's database session while nginx stayed unchanged. No test password or bearer key appeared in nginx logs. Results and cleanup evidence are `/tmp/lite-cdp-nginx-compose.result.json`, `.summary.log`, `.runtime.log` and `.cleanup.log`.

A second fresh project passed the existing `bash docs/smoke-test.sh http://127.0.0.1:29585`. It exercised the real tracker, valid collection requests, origin rejection, pageviews/custom events, identity merging, labels/segments, export, deletion and rotated-key rejection through nginx. Evidence: `/tmp/lite-cdp-nginx-domain-smoke.log`, `/tmp/lite-cdp-nginx-domain-smoke.result.json` and `/tmp/lite-cdp-nginx-domain-summary.log`.

Both harnesses removed only their owned containers, networks, fresh volumes and temporary overrides/cookie files. These are local HTTP/protocol/browser checks; they do not claim production TLS or live provider callbacks.
