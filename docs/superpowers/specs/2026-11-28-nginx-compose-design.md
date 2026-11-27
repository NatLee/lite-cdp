# nginx and unified Docker Compose deployment

The user requested nginx in front of the frontend and backend, managed in one Compose YAML per repository. This extends the controlled-registration delivery; account policy and first-login behavior remain unchanged.

## Architecture

nginx is the only public application service. It serves built frontend assets and proxies API routes to the existing nonroot backend on the internal Compose network. Existing standalone images remain supported. Existing host ports and named SQLite volumes are preserved.

A second static-server container adds no useful boundary, so nginx serves the assets directly. Sentiment keeps its server-owned entry-page redirects and uses an optional `inference` profile for the legacy model and worker.

## Routing and security contract

- One root `docker-compose.yml`; only nginx publishes the application port. Feedback's optional mail-test UI retains its loopback-only port.
- React uses SPA deep-link fallback and explicit missing-asset 404 responses. Proxy `/api`, `/oauth`, `/.well-known` and `/mcp` without rewriting paths. Preserve Feedback's public forms and CDP's collection/tracker routes.
- ES serves the compiler workbench; `.mjs` files require a JavaScript MIME type. Sentiment serves `/static` directly and proxies server-owned entry pages and archive allowlisting.
- Preserve Host including port, Origin, Cookie and Authorization. Replace untrusted forwarding headers with connection information. Canonical public URLs must match the browser origin; legacy URL variables remain compatible and explicit conflicts remain visible to existing validation.
- Preserve static security headers without duplicating upstream response headers. Docker DNS re-resolution follows backend container IP changes.
- Use HTTP/1.1, disable proxy/request buffering, and allow 300-second reads/writes for streaming. Apply a bounded connect timeout.
- Log paths without query strings or credentials. Disable server-context error logging because nginx can append full request queries even at critical severity; retain main-context startup diagnostics and safe status logs.
- ES and Sentiment browser setup through nginx still requires a deployment setup token. Forwarded loopback headers must not bypass this guard.
- Sentiment defaults to web plus nginx, without GPU or worker credentials. The optional inference worker validates its secret at startup and has an explicit build dependency on the model image. Preserve mounts and `access-data`; remove the obsolete second Compose YAML.

## Verification and delivery

Validate Compose configuration, original volumes, one public application service, nginx syntax, image builds and isolated runtime flows. Through nginx, test first-login, closed/approval/open registration, review, cookies, Origin rejection, canonical OAuth metadata, bearer API and official MCP transport. Test missing assets, forwarding headers, query redaction during upstream failures and changed backend IP recovery. Preserve special public routes.

Record real browser/HTTP evidence in `docs/nginx-deployment.md`; do not claim live Google/GitHub or NVIDIA execution from local fixture tests. Update README architecture and commands. After independent review and applicable checks, create PRs, merge accepted heads and delete task branches under the user's existing authorization. Preserve repository visibility and remove only owned test resources.

## References

- https://nginx.org/en/docs/http/ngx_http_proxy_module.html
- https://docs.docker.com/compose/how-tos/profiles/
- https://docs.docker.com/reference/compose-file/services/
