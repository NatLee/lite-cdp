# First-login design — 2026-11-27

## Intent and scope
An empty installation must present a distinct /first-login page where the operator creates the first administrator and enters the application. Existing installations show normal login; administrators continue to create subsequent users. Public self-registration is not part of the default scope; user clarification is pending and will amend this explicitly if requested. This continues the user's authorized direct implementation and PR/merge/branch-cleanup workflow.

## Approaches
Keeping setup as an inline login variant hides its lifecycle. A broad onboarding wizard would introduce unrelated setup work. Chosen implementation: a dedicated first-login route with a concise themed introduction and one account form, reusing existing domain branding and backend account/session contracts. This is the implementer's choice, consistent with the user's requested first-entry account creation.

## Behavior
- Empty installation: root/login/deep protected entries resolve to /first-login. Preserve only the existing validated local OAuth next continuation; no external redirect.
- Show first-account purpose, username, optional existing display-name/email fields, password and confirmation. Preserve server field constraints; validate mismatch before POST. Clear sensitive inputs after success, avoid storage/logs/URL secrets, disable duplicate submissions, expose accessible errors and busy state.
- Success creates exactly one administrator using existing atomic setup logic, sets the existing session cookie, and enters the normal authenticated app (or valid OAuth continuation).
- Existing installation: /first-login resolves to normal login or authenticated home. On concurrent initialization conflict, refresh setup state and transition to login; do not retain a stale setup form.
- Keep existing admin user-management pages. Do not silently enable third-party self-registration or change role/domain permissions.
- ES and Sentiment preserve loopback setup. Nonloopback/container setup may supply ACCESS_SETUP_TOKEN from deployment configuration (minimum32 characters), verified in constant time; keep host/origin and atomic empty-database guards. The token is never returned by state API or logged. State exposes only whether a token is required/configured. Existing CLI bootstrap stays available.

## Visual design
Reuse each project's existing palette, typography and themed introduction; add a first-run identity/steps section and clear form section. Four-pixel spacing,400/600 weights, small radii, no gradients/glass/emoji. Preserve keyboard, light/dark,390px and reduced-motion support. Use actual new screenshots from an empty disposable database; update README setup instructions and link evidence.

## Verification
Cover empty/existing/authenticated entry, confirmation mismatch, success, setup409 races, safe continuation, server failures and repeated submissions. For token-protected hosts also prove missing/wrong/correct token, loopback compatibility, canonical origin, existing-account refusal and concurrent first-create uniqueness. Run affected full suites/builds, actual browser fresh-database setup→authenticated→logout→login, and mobile/desktop checks. Root runs independent final review then authorized PR/CI/merge/delete workflow.
