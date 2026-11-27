# Signal Console implementation plan

1. Add failing API regressions for collection policy, stable subject identity, replay protection and isolated installation receipts. Add tracker browser-like tests for consent and loader lifecycle.
2. Add additive migration; policy validation/sanitization; site APIs/config; deterministic identity and event metadata/idempotency; receipt and signed-subject boundaries. Preserve legacy payloads and public operation registry.
3. Replace tracker with policy-aware singleton lifecycle, site storage migration, cross-tab reset/revoke, consent gating, bounded retries, history/hash/manual modes and diagnostics.
4. Add installation workbench: checked framework templates, policy editor, real receipt steps and synthetic identity simulator. Update people presentation for user-ID-only identities.
5. Implement graphite/chalk/olive visual system across existing screens, editable cover and current README teaching.
6. Run full suites/build/format, real Chrome integration and light/dark/mobile capture. Record exact evidence and remaining limitations. Review diff before scoped local commit; no remote actions.

Progress is recorded in `docs/signal-console-verification.md`.

All six implementation steps are complete. Local suite/build/browser/Compose evidence is recorded in the verification document; parent review and any remote release remain separate.

Review followup: both identity-ordering and loader-initialization blockers reproduced with failing tests, fixed, and independently re-reviewed. Expanded real-browser and Compose checks cover delayed first delivery, shared clocks, reloads, IndexedDB fallback and onload commands.
